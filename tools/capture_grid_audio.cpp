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
static int run(int argc, wchar_t** argv) {
    if(argc!=5) throw std::runtime_error("usage: capture-grid-audio PID START_FILETIME DURATION_MS local/NEW.wav");
    auto process_id = number(argv[1]); auto expected_start = number(argv[2]); auto duration = number(argv[3]);
    if(!process_id || process_id>MAXDWORD || duration<500 || duration>60000) throw std::runtime_error("capture argument outside bound");
    auto output = private_path(argv[4]);
    auto metadata = private_path((output.native()+L".packets.tsv").c_str());
    Handle process{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|SYNCHRONIZE,FALSE,static_cast<DWORD>(process_id))};
    if(!process.value) throw std::runtime_error("target process unavailable");
    wchar_t path[32768]; DWORD size = 32768;
    FILETIME created{}, exited{}, kernel{}, user{};
    if(!QueryFullProcessImageNameW(process.value,0,path,&size) || _wcsicmp(path,L"C:\\Users\\debros\\Tools\\grid-research\\grid-compat\\grid.exe") || !GetProcessTimes(process.value,&created,&exited,&kernel,&user) || filetime(created)!=expected_start) throw std::runtime_error("private GRID process identity mismatch");
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
    if(wav.value==INVALID_HANDLE_VALUE || packets.value==INVALID_HANDLE_VALUE) throw std::runtime_error("exclusive private output creation failed");
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
    std::printf("audio_capture event=completed frames=%zu packets=%u discontinuities=%u timestamp_errors=%u process_tree_only=1\n",pcm.size()/4,count,discontinuities,timestamp_errors);
    if(!count) throw std::runtime_error("capture produced no audio packets");
    return 0;
}
int wmain(int argc,wchar_t** argv) {
    try{return run(argc,argv);}catch(const std::exception& error){std::fprintf(stderr,"capture-grid-audio: %s\n",error.what());return 1;}
}
