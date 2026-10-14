// Bounded process-only Windows loopback capture, independently authored.
// No microphone, whole-desktop fallback, game injection or settings changes.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <audioclient.h>
#include <audioclientactivationparams.h>
#include <mmdeviceapi.h>
#include <wrl.h>
#include <filesystem>
#include <vector>
#include <string>
#include <cstdio>
#include <cstdlib>
#include <cwctype>
#include <cstdint>
#include <cstring>
#include <array>
#include <memory>
using Microsoft::WRL::ComPtr;
using namespace Microsoft::WRL;
class Activation final : public RuntimeClass<RuntimeClassFlags<ClassicCom>, IActivateAudioInterfaceCompletionHandler, FtmBase> {
public:
    HANDLE ready = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    HRESULT result = E_PENDING;
    ComPtr<IAudioClient> client;
    ~Activation() { if(ready) CloseHandle(ready); }
    HRESULT STDMETHODCALLTYPE ActivateCompleted(IActivateAudioInterfaceAsyncOperation* operation) override {
        HRESULT status = E_UNEXPECTED;
        ComPtr<IUnknown> unknown;
        result = operation->GetActivateResult(&status, &unknown);
        if(SUCCEEDED(result)) result = status;
        if(SUCCEEDED(result)) result = unknown.As(&client);
        SetEvent(ready);
        return S_OK;
    }
};
struct Handle {
    HANDLE value = INVALID_HANDLE_VALUE;
    ~Handle() { if(value != INVALID_HANDLE_VALUE && value) CloseHandle(value); }
};
static uint64_t number(const wchar_t* text) {
    wchar_t* end = nullptr; auto value = _wcstoui64(text, &end, 10);
    if(!*text || !end || *end) throw std::runtime_error("invalid unsigned argument");
    return value;
}
static uint64_t filetime(FILETIME value) { return uint64_t(value.dwLowDateTime) | uint64_t(value.dwHighDateTime) << 32; }
static void check(HRESULT hr, const char* action) {
    if(FAILED(hr)) { std::fprintf(stderr,"audio_capture action=%s hresult=%08lx\n",action,static_cast<unsigned long>(hr)); throw std::runtime_error(action); }
}
static void write(HANDLE handle, const void* data, DWORD bytes) {
    DWORD done = 0;
    if(!WriteFile(handle,data,bytes,&done,nullptr) || done!=bytes) throw std::runtime_error("write failed");
}
static std::filesystem::path private_path(const wchar_t* text) {
    auto root = std::filesystem::canonical(L"local");
    auto target = std::filesystem::weakly_canonical(text);
    auto relative = target.lexically_relative(root);
    if(relative.empty() || relative.native().starts_with(L"..") || std::filesystem::exists(target) || !std::filesystem::is_directory(target.parent_path())) throw std::runtime_error("new private local output required");
    return target;
}
static void word(std::vector<unsigned char>& header, uint32_t value, int bytes) {
    for(int i=0;i<bytes;i++) header.push_back(static_cast<unsigned char>(value >> (8*i)));
}
// Optional ET02 observation: the controller has already been found by the
// separately identity-checked reader. Reads are sequential, never atomic.
class EngineTrace {
    HANDLE process_;
    uint32_t controller_, root_, component_, vehicle_, physical_, camera_;
    static uint32_t get(const unsigned char* bytes, size_t offset) {
        uint32_t value;std::memcpy(&value,bytes+offset,4);return value;
    }
    void read(uint32_t address, void* destination, size_t length) {
        SIZE_T done=0;
        if(address<0x10000 || uint64_t(address)+length>0x7fff0000 || length>0x340 ||
           !ReadProcessMemory(process_,reinterpret_cast<const void*>(uintptr_t(address)),destination,length,&done) || done!=length)
            throw std::runtime_error("bounded engine observation read failed");
    }
    uint32_t pointer(uint32_t address) {uint32_t value=0;read(address,&value,4);return value;}
public:
    static constexpr uint32_t record_bytes=5088;
    std::vector<unsigned char> bytes;
    uint32_t count=0;
    EngineTrace(HANDLE process,uint32_t controller,uint32_t pid):process_(process),controller_(controller) {
        std::array<unsigned char,0x340> state{};read(controller_,state.data(),state.size());
        root_=get(state.data(),0x128);component_=get(state.data(),0x110);
        if(uint64_t(root_)+0x8f10!=controller_ || get(state.data(),0x118)!=root_+0x1a40 ||
           pointer(root_+0x20)!=component_ || pointer(component_+0x6c)!=0 ||
           get(state.data(),0x328)!=477036 || get(state.data(),0x32c)!=319292)
            throw std::runtime_error("player Lacetti engine ownership mismatch");
        vehicle_=pointer(component_+0x24);
        physical_=pointer(pointer(vehicle_+0xdb0)+8);
        camera_=pointer(get(state.data(),0x114)+0x28);
        for(char c:std::string("ET02"))bytes.push_back(static_cast<unsigned char>(c));
        word(bytes,record_bytes,4);word(bytes,0,4);word(bytes,controller_,4);
        LARGE_INTEGER frequency{};QueryPerformanceFrequency(&frequency);
        word(bytes,static_cast<uint32_t>(frequency.QuadPart),4);word(bytes,static_cast<uint32_t>(frequency.QuadPart>>32),4);
        word(bytes,pid,4);word(bytes,0,4);
        bytes.reserve(32+6500*size_t(record_bytes));
    }
    void sample() {
        if(count>=6500)throw std::runtime_error("engine observation count bound exceeded");
        std::array<unsigned char,record_bytes> row{};
        LARGE_INTEGER begin{},end{};QueryPerformanceCounter(&begin);
        std::memcpy(row.data(),&begin.QuadPart,8);
        auto state=row.data()+16;
        read(controller_,state,0x340);
        if(get(state,0x128)!=root_ || get(state,0x110)!=component_ || get(state,0x118)!=root_+0x1a40 ||
           pointer(component_+0x24)!=vehicle_ || pointer(pointer(vehicle_+0xdb0)+8)!=physical_ ||
           pointer(get(state,0x114)+0x28)!=camera_ || get(state,0x328)!=477036 || get(state,0x32c)!=319292)
            throw std::runtime_error("engine observation ownership changed");
        size_t at=16+0x340;
        for(uint32_t offset:{0x1894u,0x17a4u,0x17a8u}) {read(vehicle_+offset,row.data()+at,4);at+=4;}
        read(camera_+0xc1c,row.data()+at,4);at+=4;
        read(root_+0xa6a4,row.data()+at,4);at+=4;
        read(physical_+0x210,row.data()+at,32);at+=32;
        constexpr size_t voices_at=900;
        for(uint32_t i=0;i<12;i++) {read(get(state,0x1ec+i*4),row.data()+at,0xe0);at+=0xe0;}
        auto slot_limit=pointer(0xebf434);
        if(slot_limit>4096)throw std::runtime_error("backend voice table outside bound");
        for(uint32_t i=0;i<12;i++) {
            auto slot=get(row.data()+voices_at+i*0xe0,0x68);
            if(slot!=0xa000001) {
                if(slot>=slot_limit)throw std::runtime_error("backend voice slot outside bound");
                read(0xe9c7f0+slot*0x370+0x300,row.data()+at,0x70);
            }
            at+=0x70;
        }
        std::array<unsigned char,0x340> repeat{};read(controller_,repeat.data(),repeat.size());
        uint32_t exact=std::memcmp(state,repeat.data(),repeat.size())==0?1:0;
        std::memcpy(row.data()+at,&exact,4);at+=4;
        read(0xebf420,row.data()+at,64);at+=64;
        read(0xebee28,row.data()+at,36);at+=36;
        read(0xe50900,row.data()+at,16);at+=16;
        read(0xebedf4,row.data()+at,36);
        QueryPerformanceCounter(&end);std::memcpy(row.data()+8,&end.QuadPart,8);
        bytes.insert(bytes.end(),row.begin(),row.end());++count;
    }
    void finish() {std::memcpy(bytes.data()+8,&count,4);}
};
static int run(int argc, wchar_t** argv) {
    if(argc!=5 && argc!=6) throw std::runtime_error("usage: capture-grid-audio PID START_FILETIME DURATION_MS local/NEW.wav [PLAYER_CONTROLLER_DECIMAL]");
    auto process_id = number(argv[1]); auto expected_start = number(argv[2]); auto duration = number(argv[3]);
    if(!process_id || process_id>MAXDWORD || duration<500 || duration>60000) throw std::runtime_error("capture argument outside bound");
    auto output = private_path(argv[4]);
    auto metadata = private_path((output.native()+L".packets.tsv").c_str());
    auto controller=argc==6?number(argv[5]):0;
    if(argc==6 && (controller<0x20000 || controller>0x7ffe0000))throw std::runtime_error("engine controller outside bound");
    std::filesystem::path engine_output;
    if(controller)engine_output=private_path((output.native()+L".engine.bin").c_str());
    Handle process{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|SYNCHRONIZE|(controller?PROCESS_VM_READ:0),FALSE,static_cast<DWORD>(process_id))};
    if(!process.value) throw std::runtime_error("target process unavailable");
    wchar_t path[32768]; DWORD size = 32768;
    FILETIME created{}, exited{}, kernel{}, user{};
    if(!QueryFullProcessImageNameW(process.value,0,path,&size) || _wcsicmp(path,L"C:\\Users\\debros\\Tools\\grid-research\\grid-compat\\grid.exe") || !GetProcessTimes(process.value,&created,&exited,&kernel,&user) || filetime(created)!=expected_start) throw std::runtime_error("private GRID process identity mismatch");
    std::unique_ptr<EngineTrace> engine;
    if(controller)engine=std::make_unique<EngineTrace>(process.value,static_cast<uint32_t>(controller),static_cast<uint32_t>(process_id));
    check(CoInitializeEx(nullptr,COINIT_MULTITHREADED),"COM initialization");
    struct ComScope { ~ComScope(){CoUninitialize();} } com_scope;
    auto activation = Make<Activation>();
    if(!activation || !activation->ready) throw std::runtime_error("activation event unavailable");
    AUDIOCLIENT_ACTIVATION_PARAMS parameters{};
    parameters.ActivationType = AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK;
    parameters.ProcessLoopbackParams.TargetProcessId = static_cast<DWORD>(process_id);
    parameters.ProcessLoopbackParams.ProcessLoopbackMode = PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE;
    PROPVARIANT variant{}; variant.vt = VT_BLOB;
    variant.blob.cbSize = sizeof(parameters); variant.blob.pBlobData = reinterpret_cast<BYTE*>(&parameters);
    ComPtr<IActivateAudioInterfaceAsyncOperation> operation;
    check(ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,__uuidof(IAudioClient),&variant,activation.Get(),&operation),"process loopback activation");
    if(WaitForSingleObject(activation->ready,5000)!=WAIT_OBJECT_0) throw std::runtime_error("audio activation timeout");
    check(activation->result,"audio activation result");
    WAVEFORMATEX format{};format.wFormatTag=WAVE_FORMAT_PCM;format.nChannels=2;format.nSamplesPerSec=48000;format.wBitsPerSample=16;format.nBlockAlign=4;format.nAvgBytesPerSec=192000;
    check(activation->client->Initialize(AUDCLNT_SHAREMODE_SHARED,AUDCLNT_STREAMFLAGS_LOOPBACK|AUDCLNT_STREAMFLAGS_EVENTCALLBACK|AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,0,0,&format,nullptr),"audio format initialization");
    UINT32 capacity = 0;check(activation->client->GetBufferSize(&capacity),"audio buffer capacity");
    if(!capacity || capacity>480000) throw std::runtime_error("audio buffer capacity outside bound");
    ComPtr<IAudioCaptureClient> capture;check(activation->client->GetService(IID_PPV_ARGS(&capture)),"capture interface");
    Handle samples{CreateEventW(nullptr,FALSE,FALSE,nullptr)};
    if(!samples.value) throw std::runtime_error("sample event unavailable");
    check(activation->client->SetEventHandle(samples.value),"sample event binding");
    Handle wav{CreateFileW(output.c_str(),GENERIC_WRITE,0,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr)};
    Handle packets{CreateFileW(metadata.c_str(),GENERIC_WRITE,0,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr)};
    Handle engine_file;
    if(engine)engine_file.value=CreateFileW(engine_output.c_str(),GENERIC_WRITE,0,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr);
    if(wav.value==INVALID_HANDLE_VALUE || packets.value==INVALID_HANDLE_VALUE) throw std::runtime_error("exclusive private output creation failed");
    if(engine && engine_file.value==INVALID_HANDLE_VALUE)throw std::runtime_error("exclusive engine observation creation failed");
    std::vector<unsigned char> pcm;pcm.reserve(static_cast<size_t>((duration*48+capacity)*4));
    std::string trace = "packet\tframes\tflags\tdevice_position\tqpc_100ns\n";
    LARGE_INTEGER began{}, frequency{};FILETIME utc{};
    QueryPerformanceFrequency(&frequency);QueryPerformanceCounter(&began);GetSystemTimeAsFileTime(&utc);
    check(activation->client->Start(),"capture start");
    struct Stop { IAudioClient* client; ~Stop(){client->Stop();} } stop{activation->client.Get()};
    std::printf("audio_capture event=started pid=%lu utc_filetime=%llu qpc=%lld qpc_frequency=%lld process_tree_only=1 rate=48000 channels=2\n",static_cast<unsigned long>(process_id),static_cast<unsigned long long>(filetime(utc)),began.QuadPart,frequency.QuadPart); std::fflush(stdout);
    uint32_t count=0, discontinuities=0, timestamp_errors=0;
    for(;;) {
        LARGE_INTEGER now{};QueryPerformanceCounter(&now);
        if((now.QuadPart-began.QuadPart)*1000/frequency.QuadPart >= static_cast<LONGLONG>(duration)) break;
        HANDLE waits[]{samples.value,process.value};DWORD waited=WaitForMultipleObjects(2,waits,FALSE,1000);
        if(waited==WAIT_OBJECT_0+1) throw std::runtime_error("GRID exited during capture");
        if(waited==WAIT_FAILED) throw std::runtime_error("capture wait failed");
        if(engine)engine->sample();
        UINT32 available=0;check(capture->GetNextPacketSize(&available),"packet size");
        while(available) {
            BYTE* data=nullptr;UINT32 frames=0;DWORD flags=0;UINT64 position=0,qpc=0;
            check(capture->GetBuffer(&data,&frames,&flags,&position,&qpc),"sample buffer");
            if(frames>capacity || pcm.size()+size_t(frames)*4>size_t(65)*192000) {capture->ReleaseBuffer(frames);throw std::runtime_error("capture sample bound exceeded");}
            auto old=pcm.size();pcm.resize(old+size_t(frames)*4,0);
            if(!(flags&AUDCLNT_BUFFERFLAGS_SILENT) && data) std::memcpy(pcm.data()+old,data,size_t(frames)*4);
            check(capture->ReleaseBuffer(frames),"sample release");
            if(flags&AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY) ++discontinuities;
            if(flags&AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR) ++timestamp_errors;
            trace+=std::to_string(count++)+"\t"+std::to_string(frames)+"\t"+std::to_string(flags)+"\t"+std::to_string(position)+"\t"+std::to_string(qpc)+"\n";
            check(capture->GetNextPacketSize(&available),"next packet size");
        }
    }
    check(activation->client->Stop(),"capture stop");
    std::vector<unsigned char> header;
    for(char c:std::string("RIFF")) header.push_back(static_cast<unsigned char>(c));word(header,static_cast<uint32_t>(pcm.size())+36,4);
    for(char c:std::string("WAVEfmt ")) header.push_back(static_cast<unsigned char>(c));word(header,16,4);word(header,1,2);word(header,2,2);word(header,48000,4);word(header,192000,4);word(header,4,2);word(header,16,2);
    for(char c:std::string("data")) header.push_back(static_cast<unsigned char>(c));word(header,static_cast<uint32_t>(pcm.size()),4);
    write(wav.value,header.data(),static_cast<DWORD>(header.size()));if(!pcm.empty())write(wav.value,pcm.data(),static_cast<DWORD>(pcm.size()));
    write(packets.value,trace.data(),static_cast<DWORD>(trace.size()));FlushFileBuffers(wav.value);FlushFileBuffers(packets.value);
    if(engine){engine->finish();write(engine_file.value,engine->bytes.data(),static_cast<DWORD>(engine->bytes.size()));FlushFileBuffers(engine_file.value);std::printf("audio_capture engine_records=%u sequential_reads=1\n",engine->count);}
    std::printf("audio_capture event=completed frames=%zu packets=%u discontinuities=%u timestamp_errors=%u process_tree_only=1\n",pcm.size()/4,count,discontinuities,timestamp_errors);
    if(!count) throw std::runtime_error("capture produced no audio packets");
    return 0;
}
int wmain(int argc,wchar_t** argv) {
    try{return run(argc,argv);}catch(const std::exception& error){std::fprintf(stderr,"capture-grid-audio: %s\n",error.what());return 1;}
}
