/* Execute only the observed inline four-region loop A0F4A0..A0F507.
   Trap-flag stepping stops BEFORE its following document-constructor call.
   No original instructions are copied, replaced, or skipped within the loop. */
static DWORD wheel_thread;
static uintptr_t wheel_start,wheel_stop,wheel_resume;
static unsigned wheel_steps,wheel_stopped,wheel_error;
static LONG CALLBACK wheel_step_handler(EXCEPTION_POINTERS *e){
    if(GetCurrentThreadId()!=wheel_thread)return EXCEPTION_CONTINUE_SEARCH;
    uintptr_t ip=e->ContextRecord->Eip;
    if(e->ExceptionRecord->ExceptionCode!=EXCEPTION_SINGLE_STEP){fprintf(stderr,"wheel_loop_exception=%08lx ip=%08lx steps=%u\n",e->ExceptionRecord->ExceptionCode,(DWORD)ip,wheel_steps);return EXCEPTION_CONTINUE_SEARCH;}
    ++wheel_steps;
    if(ip==wheel_stop){wheel_stopped=1;e->ContextRecord->Eip=(DWORD)wheel_resume;e->ContextRecord->EFlags&=~0x100;return EXCEPTION_CONTINUE_EXECUTION;}
    if(ip<wheel_start||ip>=wheel_stop||wheel_steps>256){wheel_error=1;e->ContextRecord->Eip=(DWORD)wheel_resume;e->ContextRecord->EFlags&=~0x100;}
    else e->ContextRecord->EFlags|=0x100; // Windows clears TF on exception delivery.
    return EXCEPTION_CONTINUE_EXECUTION;
}
__declspec(naked) static void __cdecl wheel_region_loop(void *body,void *entry){
    __asm {
        push ebp
        mov ebp,esp
        push ebx
        push esi
        push edi
        and esp,0xfffffff0
        sub esp,0x60
        mov ebx,body
        lea esi,[ebx+0x14e0]
        lea edi,[ebx+0x1540]
        lea edx,[ebx+0x1a70]
        mov [esp+0x20],edx
        lea eax,resume_point
        mov wheel_resume,eax
        xor eax,eax
        xor ecx,ecx
        xorps xmm3,xmm3
        pushfd
        or dword ptr [esp],0x100
        popfd
        jmp dword ptr entry
    resume_point:
        mov ebx,[ebp-4]
        mov esi,[ebp-8]
        mov edi,[ebp-12]
        mov esp,ebp
        pop ebp
        ret
    }
}
static int original_wheel_regions(unsigned char *image,void *body){
    wheel_thread=GetCurrentThreadId();wheel_start=(uintptr_t)(image+0xa0f4a0-0x400000);wheel_stop=(uintptr_t)(image+0xa0f507-0x400000);
    wheel_steps=wheel_stopped=wheel_error=0;
    PVOID handler=AddVectoredExceptionHandler(1,wheel_step_handler);if(!handler)return 0;
    wheel_region_loop(body,(void *)wheel_start);
    if(!RemoveVectoredExceptionHandler(handler))wheel_error=1;
    printf("original_wheel_region_loop_steps=%u stopped_before_document_call=%u loop_error=%u\n",wheel_steps,wheel_stopped,wheel_error);
    return wheel_stopped&&!wheel_error;
}
