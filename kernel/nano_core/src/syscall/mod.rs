//! Code for initializing and handling syscalls from userspace,
//! using the amd64 SYSCALL/SYSRET special functions.


//! To invoke from userspace: 
//! The syscall number is passed in the rax register. 
//! The parameters are in this order:  rdi, rsi, rdx, r10, r8, r9. 
//! The call is invoked with the "syscall" instruction. 
//! The syscall overwrites the rcx register. 
//! The return value is in rax.


use core::sync::atomic::{Ordering, compiler_fence};
use interrupts::{AvailableSegmentSelector, get_segment_selector};
use collections::string::String;



// #[no_mangle]
fn syscall_dispatcher(syscall_number: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64, arg6: u64) -> u64{
    trace!("syscall_dispatcher: num={} arg1={} arg2={} arg3={} arg4={} arg5={} arg6={}",
            syscall_number, arg1, arg2, arg3, arg4, arg5, arg6);
    let mut result = 0xDEADBEEF01234567;

    match syscall_number{
        1 => syssend!(format!("{}", arg1), format!("{}",arg2), format!("{}", arg3)),
        2 => result = sysrecv!(format!("{}", arg1)),           
        _ => trace!("Invalid syscall {}", syscall_number),
    }
                
 /*   unsafe{
        match syscall_number{
            1 => {
                   asm!("
          mov rax, [rdx];"
          
          : : : "memory" : "intel", "volatile");

            },
            _ => trace!("Invalid syscall {}", syscall_number),
        }
    }*/
    return result;    
}


pub fn init(privilege_stack_top_usable: usize) {
    enable_syscall_sysret(privilege_stack_top_usable);
}



/// The structure that holds data related to syscall/sysret handling.
/// NOTE: DO NOT CHANGE THE ORDER OF THESE ELEMENTS, THE syscall_handler() REQUIRES THEM TO BE IN A CERTAIN ORDER.
#[repr(C)]
struct UserTaskGsData {
    // TODO: change this to a proper TLS data structure later, and then swap the current GS to it in the task switcher

    /// the kernel's rsp
    kernel_stack: u64, // offset 0x0 (0)
    /// the user task's rsp, which is found in rsp upon syscall entry and should be placed back into rsp upon sysret
    user_stack: u64, // offset 0x8 (8)
    /// the user task's instruction pointer, which is found in rcx upon syscall entry and should be placed back into rcx upon sysret
    user_ip: u64, // offset 0x10 (16)
    /// the user task's rflags, which is found in r11 upon syscall entry and should be placed back into r11 upon sysret
    user_flags: u64, // offset 0x18 (24)
}





#[allow(private_no_mangle_fns)]
#[no_mangle]
#[naked]
unsafe extern "C" fn syscall_handler() {


    // switch to the kernel stack dedicated for syscall handling, and save the user task's details
    // link to similar features in tifflin: https://github.com/thepowersgang/rust_os/blob/deb156d263e0a0af9195955cccfc150ea12f466f/Kernel/Core/arch/amd64/start.asm#L335
    // here, rcx = user task's IP, r11 = user task's EFLAGS
    // The gs offsets used below must match the order of elements in the UserTaskGsData struct above!!!
    asm!("swapgs; \
          mov gs:[0x8],  rsp; \
          mov gs:[0x10], rcx; \
          mov gs:[0x18], r11; \
          mov rsp, gs:[0x0];"
          : : : "memory" : "intel", "volatile");

 /*unsafe{
    let rdi:u64;
    asm!("mov rax, rdi": : : "memory" : "intel", "volatile");
    asm!("" : "={rax}"(rdi): : "memory" : "intel", "volatile");
    trace!("The sender is {}", rdi);
}*/
    // asm!("push r11" : : : : "intel"); // stack must be 16-byte aligned, so just pushing another random item so we push an even number of things
    let (rax, rdi, rsi, rdx, r10, r8, r9): (u64, u64, u64, u64, u64, u64, u64); 
    asm!("" : "={rax}"(rax), "={rdi}"(rdi), "={rsi}"(rsi), "={rdx}"(rdx), "={r10}"(r10), "={r8}"(r8), "={r9}"(r9)  : : "memory" : "intel", "volatile");
    compiler_fence(Ordering::SeqCst);

   
    // here: once the stack is set up and registers are saved and remapped to local rust vars, then we can do anything we want
    // asm!("sti"); // TODO: we could consider letting interrupts occur while in a system call. Probably should do that. 
    
    let curr_id = ::task::get_current_task_id();
    trace!("syscall_handler: curr_tid={}  rax={:#x} rdi={:#x} rsi={:#x} rdx={:#x} r10={:#x} r8={:#x} r9={:#x}",
           curr_id, rax, rdi, rsi, rdx, r10, r8, r9);


    // FYI, Rust's calling conventions is as follows:  RDI,  RSI,  RDX,  RCX,  R8,  R9,  R10,  others on stack
    // because we have 7 args here, the last one will be  placed onto the stack, so we cannot rely on the stack not being changed.
    let result: u64 = syscall_dispatcher(rax, rdi, rsi, rdx, r10, r8, r9); 



    // below here, we cannot do anything we want, we must restore userspace registers in an atomic fashion
    compiler_fence(Ordering::SeqCst);
    // asm!("cli");
    asm!("mov rax, $0" : : "r"(result) : : "intel", "volatile"); //  put result in rax for returning to userspace

    // we don't need to save the current kernel rsp back into the UserTaskGsData struct's kernel_stack member (gs:[0x0]), 
    // because we can just re-use the same rsp that was originally placed into TSS RSP0 (which is set on a context switch)
    asm!("
          mov rsp, gs:[0x8];  \
          mov rcx, gs:[0x10]; \
          mov r11, gs:[0x18];"
          : : : "memory" : "intel", "volatile");


    // restore current GS back into GSBASE
    asm!("swapgs");
    asm!("sysretq");

}


/// Configures and enables the usage and behavior of `syscall` and `sysret` instructions. 
fn enable_syscall_sysret(privilege_stack_top_usable: usize) {

    // set up GS segment using its MSR, it should point to a special kernel stack that we can use for this.
    // Right now we're just using the save privilege level stack used for interrupts from user space (TSS's rsp 0)
    // in the future, this will be a separate value per-thread, using thread-local storage
    use x86_64::registers::msr::{IA32_GS_BASE, IA32_KERNEL_GS_BASE, IA32_FMASK, IA32_STAR, IA32_LSTAR, wrmsr};
    use alloc::boxed::Box;
    let gs_data: UserTaskGsData = UserTaskGsData {
        kernel_stack: privilege_stack_top_usable as u64,
        // the other 3 elements below are 0, but will be init'd at the entry of every syscall_handler invocation
        user_stack: 0,
        user_ip: 0,
        user_flags: 0, 
    };
    let gs_data_ptr = Box::into_raw(Box::new(gs_data)) as u64; // puts it on the kernel heap, and prevents it from being dropped
    unsafe { wrmsr(IA32_KERNEL_GS_BASE, gs_data_ptr); }
    unsafe { wrmsr(IA32_GS_BASE, gs_data_ptr); }
    debug!("Set KERNEL_GS_BASE and GS_BASE to include a kernel stack at {:#x}", privilege_stack_top_usable);
    
    // set a kernelspace entry point for the syscall instruction from userspace
    unsafe { wrmsr(IA32_LSTAR, syscall_handler as u64); }

	// set up user code segment and kernel code segment
    // I believe the cs segment below should be 0x18, not 0x1B, because it's an offset, not a true descriptor with privilege level masks. 
    //      Beelzebub (vercas) sets it as 0x18.
    let user_cs = get_segment_selector(AvailableSegmentSelector::UserCode32).0 - 3;   // FIXME: more correct to do "& (!0b11);" rather than "-3"
    let kernel_cs = get_segment_selector(AvailableSegmentSelector::KernelCode).0;   // FIXME: more correct to do "& (!0b11);" rather than "-3"
    let star_val: u32 = ((user_cs as u32) << 16) | (kernel_cs as u32); // this is what's recommended
    unsafe { wrmsr(IA32_STAR, (star_val as u64) << 32); }   //  [63:48] User CS, [47:32] Kernel CS
    debug!("Set IA32_STAR to {:#x}", star_val);

    // set up flags upon kernelspace entry into syscall_handler
    let rflags_interrupt_bitmask = 0x200;
    unsafe { wrmsr(IA32_FMASK, rflags_interrupt_bitmask); }  // clear interrupts during syscalls (if the bit is set here, it will be cleared upon a syscall)
}

