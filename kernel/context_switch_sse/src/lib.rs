//! This crate contains structures and routines for context switching 
//! when SSE/SIMD extensions are enabled. 

#![no_std]
#![feature(asm, naked_functions)]


/// The registers saved before a context switch and restored after a context switch.
#[repr(C, packed)]
pub struct Context {
    // The order of the registers here MUST MATCH the order of 
    // registers popped in the context_switch() function below. 
    xmm15: u128,
    xmm14: u128,   
    xmm13: u128,   
    xmm12: u128,   
    xmm11: u128,   
    xmm10: u128,   
    xmm9:  u128,   
    xmm8:  u128,   
    xmm7:  u128,   
    xmm6:  u128,   
    xmm5:  u128,   
    xmm4:  u128,   
    xmm3:  u128,   
    xmm2:  u128,   
    xmm1:  u128,   
    xmm0:  u128, 

    r15: usize, 
    r14: usize,
    r13: usize,
    r12: usize,
    rbp: usize,
    rbx: usize,
    rip: usize,
}

impl Context {
    pub fn new(rip: usize) -> Context {
        Context {
            xmm15: 0,
            xmm14: 0,   
            xmm13: 0,   
            xmm12: 0,   
            xmm11: 0,   
            xmm10: 0,   
            xmm9:  0,   
            xmm8:  0,   
            xmm7:  0,   
            xmm6:  0,   
            xmm5:  0,   
            xmm4:  0,   
            xmm3:  0,   
            xmm2:  0,   
            xmm1:  0,   
            xmm0:  0,   

            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbp: 0,
            rbx: 0,
            rip: rip,
        }
    }
}



/// Performs the actual switch from the previous (current) task to the next task.
/// 
/// This is the task switch routine for when SSE/SIMD extensions are enabled.
/// 
/// # Arguments
/// First argument  (put in `rdi`): mutable pointer to the previous task's stack pointer
/// Second argument (put in `rsi`): the value of the next task's stack pointer
/// 
/// # Safety
/// This function is unsafe because it changes the content on both task's stacks. 
/// Also, it must be a naked function, so there cannot be regular arguments passed into it.
/// Instead, the caller of this function must place the first argument into the `rdi` register
/// and the second argument into the `rsi` register right before invoking this function.
#[allow(private_no_mangle_fns)]
#[naked]
#[no_mangle]
#[inline(never)]
pub unsafe fn context_switch() {

    // When SSE is enabled, we need to save xmm# registers in addition to regular registers
    asm!("
        # save all general purpose registers into the previous task
        push rbx
        push rbp
        push r12
        push r13
        push r14
        push r15
        
        # save all of the xmm registers (for SSE)
        # each register is 16 bytes, and there are 16 of them
        lea rsp, [rsp - 16*16]
        movdqu [rsp + 16*0],  xmm0   # push xmm0
        movdqu [rsp + 16*1],  xmm1   # push xmm1
        movdqu [rsp + 16*2],  xmm2   # push xmm2
        movdqu [rsp + 16*3],  xmm3   # push xmm3
        movdqu [rsp + 16*4],  xmm4   # push xmm4
        movdqu [rsp + 16*5],  xmm5   # push xmm5
        movdqu [rsp + 16*6],  xmm6   # push xmm6
        movdqu [rsp + 16*7],  xmm7   # push xmm7
        movdqu [rsp + 16*8],  xmm8   # push xmm8
        movdqu [rsp + 16*9],  xmm9   # push xmm9
        movdqu [rsp + 16*10], xmm10  # push xmm10
        movdqu [rsp + 16*11], xmm11  # push xmm11
        movdqu [rsp + 16*12], xmm12  # push xmm12
        movdqu [rsp + 16*13], xmm13  # push xmm13
        movdqu [rsp + 16*14], xmm14  # push xmm14
        movdqu [rsp + 16*15], xmm15  # push xmm15
        
        # switch the stack pointers
        mov [rdi], rsp
        mov rsp, rsi

        # restore all of the xmm registers
        movdqu xmm15, [rsp + 16*15]   # pop xmm15
        movdqu xmm14, [rsp + 16*14]   # pop xmm14
        movdqu xmm13, [rsp + 16*13]   # pop xmm13
        movdqu xmm12, [rsp + 16*12]   # pop xmm12
        movdqu xmm11, [rsp + 16*11]   # pop xmm11
        movdqu xmm10, [rsp + 16*10]   # pop xmm10
        movdqu xmm9,  [rsp + 16*9]    # pop xmm9
        movdqu xmm8,  [rsp + 16*8]    # pop xmm8
        movdqu xmm7,  [rsp + 16*7]    # pop xmm7
        movdqu xmm5,  [rsp + 16*5]    # pop xmm5
        movdqu xmm6,  [rsp + 16*6]    # pop xmm6
        movdqu xmm4,  [rsp + 16*4]    # pop xmm4
        movdqu xmm3,  [rsp + 16*3]    # pop xmm3
        movdqu xmm2,  [rsp + 16*2]    # pop xmm2
        movdqu xmm1,  [rsp + 16*1]    # pop xmm1
        movdqu xmm0,  [rsp + 16*0]    # pop xmm0
        lea rsp, [rsp + 16*16]

        # restore the next task's general purpose registers
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx
        
        # pops the last value off the top of the stack,
        # so the new task's stack top must point to a target function
        ret"
        : : : "memory" : "intel", "volatile"
    );
}