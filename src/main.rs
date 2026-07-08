use std::{
    io::{self, Write},
    sync::mpsc,
    thread,
};

mod vm;

fn main() -> anyhow::Result<()> {
    // Multiple producers, single consumer kind of channel (similar to Go)
    // >= 1 threads push value in the channel, but one thread pulls the vaue out only
    let (tx, rx) = mpsc::channel();

    // ctrlc is a closure that runs sometime later (might be on another thread)
    // Move the ownership of tx from main to ctrlc
    // when tx sends ctrl + c signal to rx, rx will stop blocking main
    ctrlc::set_handler(move || {
        tx.send(()).ok();
    })?;

    // Obtain guest code as a slice of bytes

    //     xor %rax, %rax                ; Zero %rax (not necessary)                        [0x00]
    //     push %rax                     ; Pad the stack with zeros (not necessary)         [0x03]
    //     mov $0x00000a21646c726f, %rax ; Load 'o', 'r', 'l', 'd', '!', '\n', '\0', '\0'   [0x04]
    //     push %rax                     ; "orld\n\0\0" is now on the stack                 [0x0e]
    //     mov $0x57202c6f6c6c6548, %rbx ; Load 'H', 'e', 'l', 'l', 'o', ',', ' ', 'W'      [0x0f]
    //     push %rbx                     ; "Hello, W" is now on the stack                   [0x19]
    //     mov %rsp, %rsi                ; %rsi now points at the top of the stack          [0x1a]
    // .loop:
    //     mov (%rsi), %al               ; Read a byte from %rsi                            [0x1d]
    //     test %al, %al                 ; Set the flags to test if its zero                [0x1f]
    //     je .reset                     ; If it is zero we are done                        [0x21]
    //     out %al, $0x10                ; Write out the byte to I/O port 0x10              [0x23]
    //     inc %rsi                      ; Move %rsi to the next byte on the stack          [0x25]
    //     jmp .loop                     ; Do it again                                      [0x28]
    // .reset
    //     mov %rsp, %rsi                ; Reset %rsi to point back to the top of the stack [0x2a]
    //     jmp .loop                                                                        [0x2d]
    //     nop                           ; For the purposes of %rip-relative offset calcs   [0x2f]
    let guest_code = [
        0x48, 0x31, 0xc0, // [0x00]
        0x50, // [0x03]
        0x48, 0xb8, 0x6f, 0x72, 0x6c, 0x64, 0x21, 0x0a, 0x00, 0x00, // [0x04]
        0x50, // [0x0e]
        0x48, 0xbb, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, 0x57, // [0x0f]
        0x53, // [0x19]
        0x48, 0x89, 0xe6, // [0x1a]
        0x8a, 0x06, // [0x1d]
        0x84, 0xc0, // [0x1f]
        0x74, 0x07, // [0x21]
        0xe6, 0x10, // [0x23]
        0x48, 0xff, 0xc6, // [0x25]
        0xeb, 0xf3, // [0x28]
        0x48, 0x89, 0xe6, // [0x2a]
        0xeb, 0xee, // [0x2d]
        0x90, // [0x2f]
    ];

    // Run VM on another thread
    thread::spawn(move || {
        if let Ok(mut vm) = vm::VM::new() {
            vm.write_guest_code(&guest_code);
            vm.run_with_io_handler(|port, data| {
                if port == 0x10 && data.len() == 1 {
                    print!("{}", data[0] as char);
                    // Buffer flushes (bucket pushing out content) to the screen
                    io::stdout().flush().ok();
                }
            })
            .ok();
        }
    });

    // Wait until ctrl + c signal sent by tx 
    // and keep blocking main from returning
    rx.recv().ok();

    Ok(())
}
