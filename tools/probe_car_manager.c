#define GRID_CAR_MANAGER
#define GRID_HANDLING_QUERIES
#define wmain manager_main
#include "probe_car_job.c"
#undef wmain
static LONG CALLBACK manager_exception(EXCEPTION_POINTERS *p){
    fprintf(stderr,"manager_first_chance=%08lx eip=%08lx address=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip,(DWORD)p->ExceptionRecord->ExceptionInformation[1]);
    return EXCEPTION_CONTINUE_SEARCH;
}
int wmain(int argc,wchar_t **argv){
    PVOID observer=AddVectoredExceptionHandler(1,manager_exception);if(!observer)return 36;
    int result=manager_main(argc,argv);if(!RemoveVectoredExceptionHandler(observer))return 36;return result;
}
