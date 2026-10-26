use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
    ser,
};
use std::{
    cell::{Cell, RefCell},
    fmt, mem,
    os::fd::RawFd,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};
use serde::de::Error;

const KCMP_FILE: u32 = 0;

thread_local! {
    static DESCRIPTOR_SRC: RefCell<Option<Vec<Option<SafeDescriptor>>>> = Default::default();
}

thread_local! {
    /// Thread-local storage with one instance for one thread.
    // RefCell allows us to mutate the collection via shared references (common &T type).
    static DESCRIPTOR_DST: RefCell<Option<Vec<RawDescriptor>>> = Default::default();
}

pub type RawDescriptor = RawFd;

pub trait FromRawDescriptor {
    /// Convert to one descriptor type from RawDescriptor.
    /// The caller should transfer ownership of the raw descriptor to the returned object.
    /// Safe only if the caller ensures nothing has access to the descriptor
    /// after passing it to `from_raw_descriptor`
    unsafe fn from_raw_descriptor(descriptor: RawDescriptor) -> Self;
}

impl FromRawDescriptor for SafeDescriptor {
    unsafe fn from_raw_descriptor(descriptor: RawDescriptor) -> Self {
        SafeDescriptor { descriptor }
    }
}

/// Wrapper for a `Serialize` value which will capture any descriptors exported by the value.
/// NOTE: We pass in a lifetime parameter 'a for the wrapper's reference to the message and a type parameter for message.
/// The wrapper borrows the message, and the reference must NOT outlive the message.
pub struct SerializeDescriptors<'a, T: Serialize>(&'a T, Cell<Vec<RawDescriptor>>);

impl<'a, T: Serialize> SerializeDescriptors<'a, T> {
    pub fn new(inner: &'a T) -> Self {
        Self(inner, Default::default())
    }

    pub fn into_descriptors(self) -> Vec<RawDescriptor> {
        self.1.into_inner()
    }
}

impl<T: Serialize> Serialize for SerializeDescriptors<'_, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        init_descriptor_dst().map_err(ser::Error::custom)?;

        // Rust's try-catch by walking up the call stack and Drop resources.
        // After init the thread-local descriptor list, we must take and clear it,
        // even if serialization panics,
        // otherwise the local thread stays initialized in the next serialization.
        let res = catch_unwind(AssertUnwindSafe(|| self.0.serialize(serializer)));
        self.1.set(take_descriptor_dst().unwrap());
        match res {
            Ok(r) => r,
            Err(e) => resume_unwind(e),
        }
    }
}

/// Initialize the thread local storage for descriptor deserialization.
fn init_descriptor_dst() -> Result<(), &'static str> {
    // give closure access to local thread storage
    DESCRIPTOR_DST.with(|d| {
        let mut descriptors = d.borrow_mut();
        if descriptors.is_some() {
            return Err(
                "attempt to initialize descriptor destination that was already initialized",
            );
        }
        *descriptors = Some(Default::default());
        Ok(())
    })
}

/// Take the thread-local storage for descriptor deserialization.
fn take_descriptor_dst() -> Result<Vec<RawDescriptor>, &'static str> {
    match DESCRIPTOR_DST.with(|d| d.replace(None)) {
        Some(d) => Ok(d),
        None => Err("attempt to take descriptor destination before it was initialized"),
    }
}

/// Set the thread-local storage of descriptors for serialization.
fn set_descriptor_src(descriptors: Vec<Option<SafeDescriptor>>) -> Result<(), &'static str> {
    // Access thread-local data
    DESCRIPTOR_SRC.with(|d| {
        let mut src = d.borrow_mut();
        if src.is_some() {
            return Err("attempt to set descriptor source that was already set");
        }
        *src = Some(descriptors);
        Ok(())
    })
}

/// Takes the thread local storage of descriptors for deserialization
fn take_descriptor_src() -> Result<Vec<Option<SafeDescriptor>>, &'static str> {
    DESCRIPTOR_SRC.with(|d| {
        d.replace(None)
            .ok_or("attempt to take descriptor source which was never set")
    })
}

/// Allow the use of any serde deserializer within a closure (f) while providing access to a set of
/// descriptors (descriptors) for use in `deserialize_descriptor`,
/// since serialized data alone cannot carry an usuable open file or socket.
pub fn deserialize_with_descriptors<F, T, E>(
    f: F,
    descriptors: impl IntoIterator<Item = SafeDescriptor>,
) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E>,
    E: de::Error,
{
    let descriptor_src = descriptors.into_iter().map(Option::Some).collect();
    set_descriptor_src(descriptor_src).map_err(E::custom)?;

    // Ensure set_descriptor_src (init thread-local storage) is always followed by
    // take_descriptor_src (free thread-local storage) afterwards.
    let res = catch_unwind(AssertUnwindSafe(f));

    // Remove descriptors. At this point it should never panic.
    let empty_descriptors = take_descriptor_src().unwrap();

    // At this point every descriptor should be consumed.
    debug_assert!(empty_descriptors.into_iter().all(|d| d.is_none()));

    match res {
        Ok(r) => r,
        Err(e) => resume_unwind(e),
    }
}

/// Manage who owns the OS resource and when it gets closed.
/// It wraps over RawDescriptor and safely closes it when the raw descriptor falls out of scope.
#[derive(Serialize, Deserialize, Debug, Eq)]
#[serde(transparent)]
pub struct SafeDescriptor {
    #[serde(with = "with_raw_descriptor")]
    pub(crate) descriptor: RawDescriptor,
}

/// Forfeit ownership of the current raw descriptor and return the raw descriptor
pub trait IntoRawDescriptor {
    fn into_raw_descriptor(self) -> RawDescriptor;
}

impl IntoRawDescriptor for SafeDescriptor {
    fn into_raw_descriptor(self) -> RawDescriptor {
        let descriptor = self.descriptor;
        mem::forget(self);
        descriptor
    }
}

impl PartialEq for SafeDescriptor {
    fn eq(&self, other: &Self) -> bool {
        if self.descriptor == other.descriptor {
            return true;
        }

        // Safe because we only use the return value and libc ensures it is always successful
        let pid = unsafe { libc::getpid() };

        // Safe because we are passing everything by value and checking the return value
        let ret = unsafe {
            libc::syscall(
                libc::SYS_kcmp,
                pid,
                pid,
                KCMP_FILE, // Check if two fds in two processes refer to the same open fd
                self.descriptor,
                other.descriptor,
            )
        };

        ret == 0
    }
}

/// Module that exports `serialize`/`deserialize` functions for use with `#[serde(with = "...")]`
/// attribute. It only works with fields with `RawDescriptor` type.
mod with_raw_descriptor {
    use super::RawDescriptor;
    use serde::Deserializer;

    pub use super::serialize_descriptor as serialize;
    use crate::descriptor::IntoRawDescriptor;

    /// We need a deserializer D that can provide borrowed input data valid for 'de (object that
    /// borrows the data).
    /// descriptor. We pass in a lifetime parameter to satisfy serde's Deserializer trait.
    pub fn deserialize<'de, D>(de: D) -> std::result::Result<RawDescriptor, D::Error>
    where
        D: Deserializer<'de>,
    {
        // NOTE: We put this in the private mod since it needs one more step to convert to raw
        super::deserialize_descriptor(de).map(IntoRawDescriptor::into_raw_descriptor)
    }
}

/// Serialize a descriptor for later retrieval in a parent 'SerializeDescriptors' struct (a list of fds)
pub fn serialize_descriptor<S: Serializer>(
    rd: &RawDescriptor,
    se: S,
) -> std::result::Result<S::Ok, S::Error> {
    let idx = push_descriptor(*rd).map_err(ser::Error::custom)?;
    se.serialize_u32(
        idx.try_into()
            .map_err(|_| ser::Error::custom("attempt to serialize too many descriptors at once"))?,
    )
}

/// Push a descriptor on to the thread-local destination of descriptors, return the index in which
/// the descriptor was pushed to
fn push_descriptor(rd: RawDescriptor) -> Result<usize, &'static str> {
    DESCRIPTOR_DST.with(|d| {
        d.borrow_mut()
            .as_mut()
            .ok_or("attempt to serialize descriptor without descriptor destination")
            .map(|ds| {
                let idx = ds.len();
                ds.push(rd);
                idx
            })
    })
}

/// Deserializes a descriptor provided via `deserialize_with_descriptors`.
fn deserialize_descriptor<'de, D>(de: D) -> std::result::Result<SafeDescriptor, D::Error>
where
    D: Deserializer<'de>,
{
    struct DescriptorVisitor;

    impl Visitor<'_> for DescriptorVisitor {
        // One consistent output type
        type Value = u32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an integer which fits into a u32")
        }

        fn visit_u8<E: de::Error>(self, value: u8) -> Result<Self::Value, E> {
            Ok(value as _)
        }

        fn visit_u16<E: de::Error>(self, value: u16) -> Result<Self::Value, E> {
            Ok(value as _)
        }
        fn visit_u32<E: de::Error>(self, value: u32) -> Result<Self::Value, E> {
            Ok(value)
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
            value.try_into().map_err(E::custom)
        }

        fn visit_u128<E: de::Error>(self, value: u128) -> Result<Self::Value, E> {
            value.try_into().map_err(E::custom)
        }

        fn visit_i8<E: de::Error>(self, value: i8) -> Result<Self::Value, E> {
            value.try_into().map_err(E::custom)
        }

        fn visit_i16<E: de::Error>(self, value: i16) -> Result<Self::Value, E> {
            value.try_into().map_err(E::custom)
        }

        fn visit_i32<E: de::Error>(self, value: i32) -> Result<Self::Value, E> {
            value.try_into().map_err(E::custom)
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
            value.try_into().map_err(E::custom)
        }

        fn visit_i128<E: de::Error>(self, value: i128) -> Result<Self::Value, E> {
            value.try_into().map_err(E::custom)
        }
    }

    let index = de.deserialize_u32(DescriptorVisitor)? as usize;
    take_descriptor(index).map_err(D::Error::custom)
}

/// Take a descriptor at the given index from the thread-local source of descriptors
fn take_descriptor(idx: usize) -> Result<SafeDescriptor, &'static str> {
    DESCRIPTOR_SRC.with(|d| {
        d.borrow_mut()
            .as_mut()
            .ok_or("attempt to deserialize descriptor without descriptor source")?
            .get_mut(idx)
            .ok_or("attempt to deserialize out of bounds descriptor")?
            .take()
            .ok_or("attempt to deserialize descriptor that was already taken")
    })
}

#[cfg(test)]
mod tests {
    use super::deserialize_with_descriptors;
    use super::with_raw_descriptor;
    use super::{FromRawDescriptor, RawDescriptor, SafeDescriptor, SerializeDescriptors};
    use serde::{Deserialize, Serialize, de::DeserializeOwned};
    use serde_json::to_string;

    fn deserialize<T: DeserializeOwned>(json: &str, descriptors: &[RawDescriptor]) -> T {
        let safe_descriptors = descriptors
            .iter()
            .map(|&v| unsafe { SafeDescriptor::from_raw_descriptor(v) });

        deserialize_with_descriptors(|| serde_json::from_str(json), safe_descriptors).unwrap()
    }

    #[test]
    fn raw() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct RawContainer {
            #[serde(with = "with_raw_descriptor")]
            rd: RawDescriptor,
        }

        // Specifically chosen to not overlap with existing real descriptors
        let fake_rd = 5_123_457 as i32;
        let v = RawContainer {
            rd: fake_rd as RawDescriptor,
        };

        let v_serialize = SerializeDescriptors::new(&v);
        let json = to_string(&v_serialize).unwrap();
        let descriptors = v_serialize.into_descriptors();
        let res = deserialize(&json, &descriptors);
        assert_eq!(v, res)
    }
}
