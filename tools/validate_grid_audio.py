"""Independent DIC layout reconstruction and selected PCM/WAV decode audit."""
import argparse
import hashlib
import json
import struct
import sys
import wave
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'local/tools/video-python'))
import av
import numpy as np


def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    p=argparse.ArgumentParser()
    p.add_argument('debug',type=Path);p.add_argument('release',type=Path);p.add_argument('output',type=Path)
    p.add_argument('--game-root',type=Path,default=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid'))
    a=p.parse_args();assert all(q.resolve().is_relative_to((ROOT/'local').resolve()) for q in [a.debug,a.release,a.output])
    dictionary=a.game_root/'audio/CMSamples.dic';raw=dictionary.read_bytes()
    assert raw[:4]==b'DIC1' and raw[8:12]==b'2.00'
    opaque,count=struct.unpack_from('<I4xI',raw,4)
    expected=[f'P\tDIC1\t2.00\t{opaque:08x}\t{count}'];banks=[];end=16+count*24
    for i in range(count):
        start,n=struct.unpack_from('<II',raw,16+i*24);assert start==end
        name=raw[24+i*24:40+i*24].split(b'\0')[0].decode('ascii')
        extent=struct.unpack_from('<I',raw,start+n*24)[0]
        extension=raw[start+n*24+4:start+n*24+8].split(b'\0')[0].decode('ascii')
        expected.append(f'B\t{name}\t{extension}\t{extent}\t{n}');samples=[]
        for j in range(n):
            first,rate,flags=struct.unpack_from('<IHH',raw,start+j*24)
            last=struct.unpack_from('<I',raw,start+(j+1)*24)[0]
            label=raw[start+j*24+8:start+j*24+24].split(b'\0')[0].decode('ascii')
            assert 0<=first<last<=extent
            expected.append(f'S\t{name}\t{label}\t{first}\t{last}\t{rate}\t{flags:04x}')
            samples.append(dict(name=label,start=first,end=last,rate=rate,flags=flags))
        banks.append(dict(name=name,extension=extension,bytes=extent,samples=samples));end=start+n*24+8
    assert end==len(raw)
    text=('\n'.join(expected)+'\n').encode()
    assert all((folder/'dictionary.tsv').read_bytes()==text for folder in [a.debug,a.release])
    selected,=[b for b in banks if b['name']=='eng_lac'];bank=a.game_root/'audio/eng_lac.WIP';data=bank.read_bytes()
    assert selected['extension']=='WIP' and len(data)==selected['bytes']
    rows=[]
    for sample in selected['samples']:
        assert sample['flags']==0
        first=a.debug/f'{sample["name"]}.wav';second=a.release/first.name
        assert first.read_bytes()==second.read_bytes()
        with wave.open(str(first),'rb') as audio:
            assert (audio.getnchannels(),audio.getsampwidth(),audio.getframerate())==(1,2,sample['rate'])
            pcm=audio.readframes(audio.getnframes())
        prefix=data[sample['start']:sample['start']+4]
        assert prefix==b'\xff'*4
        assert pcm==data[sample['start']+4:sample['end']]
        with av.open(str(first)) as audio:
            assert audio.streams.audio[0].codec_context.name=='pcm_s16le'
            decoded=b''.join(frame.to_ndarray().astype('<i2',copy=False).tobytes() for frame in audio.decode(audio=0))
        assert decoded==pcm
        values=np.frombuffer(pcm,dtype='<i2').astype(np.float64)/32768
        rows.append(dict(name=sample['name'],non_pcm_prefix_hex=prefix.hex(),bytes=len(pcm),samples=len(values),rate=sample['rate'],seconds=len(values)/sample['rate'],rms=float(np.sqrt(np.mean(values*values))),peak=float(np.max(np.abs(values))),wav_sha256=sha(first),original_pcm_sha256=hashlib.sha256(pcm).hexdigest(),pyav_decode_word_exact=True))
    bank_sizes=[]
    for b in banks:
        path=a.game_root/'audio'/f'{b["name"]}.{b["extension"]}'
        if not path.exists():path=a.game_root/'audio/Music'/path.name
        assert path.exists() and path.stat().st_size==b['bytes'],path
        bank_sizes.append(dict(name=b['name'],bytes=b['bytes'],relative=str(path.relative_to(a.game_root))))
    paths=[Path(__file__),dictionary,bank,ROOT/'crates/grid-formats/src/audio_dictionary.rs',ROOT/'crates/grid-probes/src/bin/grid-audio-bank.rs',ROOT/'target/debug/grid-audio-bank.exe',ROOT/'target/release/grid-audio-bank.exe']
    report=dict(passed=True,banks=count,samples=sum(len(b['samples']) for b in banks),dictionary_rows=len(expected),all_bank_sizes_agree=True,bank_sizes=bank_sizes,selected_samples=rows,all_pcm_words_exact=True,native_non_pcm_prefix_bytes=4,identities={str(q):sha(q) for q in paths},pyav=av.__version__,ffmpeg_versions=av.library_versions,scope='Selected zero-flag WIP PCM16 payload after native-loader-confirmed opaque four-byte prefix. Complete remaining PCM bytes and decoded words match source. No whole original loader/mixer execution, unknown-channel decoding or auditory/fidelity review claimed.')
    with a.output.open('x') as file:json.dump(report,file,indent=2)
    print(json.dumps({k:report[k] for k in ['passed','banks','samples','dictionary_rows','all_bank_sizes_agree','all_pcm_words_exact','pyav']}))


if __name__=='__main__':main()
