//! Additional WinMM named exports for dependencies beyond CRIMSON.EXE itself.
//! Source of function-name inventory: Wine winmm.spec (names/signatures are API facts).
//! No imported WinMM function ABI is guessed: x86 tail jumps preserve the call frame.
//! Exported ORDINALS ARE NOT CANONICAL; only named imports are supported.
use std::{arch::global_asm,sync::atomic::{AtomicUsize,Ordering}};
const EXTRA:[&[u8];171]=[
    b"CloseDriver\0",
    b"DefDriverProc\0",
    b"DriverCallback\0",
    b"DrvClose\0",
    b"DrvDefDriverProc\0",
    b"DrvGetModuleHandle\0",
    b"DrvOpen\0",
    b"DrvOpenA\0",
    b"DrvSendMessage\0",
    b"GetDriverFlags\0",
    b"GetDriverModuleHandle\0",
    b"OpenDriver\0",
    b"OpenDriverA\0",
    b"PlaySound\0",
    b"PlaySoundA\0",
    b"PlaySoundW\0",
    b"SendDriverMessage\0",
    b"auxGetDevCapsW\0",
    b"auxOutMessage\0",
    b"joyConfigChanged\0",
    b"joyGetDevCapsA\0",
    b"joyGetDevCapsW\0",
    b"joyGetNumDevs\0",
    b"joyGetPos\0",
    b"joyGetPosEx\0",
    b"joyGetThreshold\0",
    b"joyReleaseCapture\0",
    b"joySetCapture\0",
    b"joySetThreshold\0",
    b"mciDriverNotify\0",
    b"mciDriverYield\0",
    b"mciExecute\0",
    b"mciFreeCommandResource\0",
    b"mciGetCreatorTask\0",
    b"mciGetDeviceIDA\0",
    b"mciGetDeviceIDFromElementIDA\0",
    b"mciGetDeviceIDFromElementIDW\0",
    b"mciGetDeviceIDW\0",
    b"mciGetDriverData\0",
    b"mciGetErrorStringW\0",
    b"mciGetYieldProc\0",
    b"mciLoadCommandResource\0",
    b"mciSendCommandW\0",
    b"mciSendStringA\0",
    b"mciSendStringW\0",
    b"mciSetDriverData\0",
    b"mciSetYieldProc\0",
    b"midiConnect\0",
    b"midiDisconnect\0",
    b"midiInAddBuffer\0",
    b"midiInClose\0",
    b"midiInGetDevCapsA\0",
    b"midiInGetDevCapsW\0",
    b"midiInGetErrorTextA\0",
    b"midiInGetErrorTextW\0",
    b"midiInGetID\0",
    b"midiInGetNumDevs\0",
    b"midiInMessage\0",
    b"midiInOpen\0",
    b"midiInPrepareHeader\0",
    b"midiInReset\0",
    b"midiInStart\0",
    b"midiInStop\0",
    b"midiInUnprepareHeader\0",
    b"midiOutCacheDrumPatches\0",
    b"midiOutCachePatches\0",
    b"midiOutClose\0",
    b"midiOutGetDevCapsA\0",
    b"midiOutGetDevCapsW\0",
    b"midiOutGetErrorTextA\0",
    b"midiOutGetErrorTextW\0",
    b"midiOutGetID\0",
    b"midiOutGetNumDevs\0",
    b"midiOutGetVolume\0",
    b"midiOutLongMsg\0",
    b"midiOutMessage\0",
    b"midiOutOpen\0",
    b"midiOutPrepareHeader\0",
    b"midiOutReset\0",
    b"midiOutSetVolume\0",
    b"midiOutShortMsg\0",
    b"midiOutUnprepareHeader\0",
    b"midiStreamClose\0",
    b"midiStreamOpen\0",
    b"midiStreamOut\0",
    b"midiStreamPause\0",
    b"midiStreamPosition\0",
    b"midiStreamProperty\0",
    b"midiStreamRestart\0",
    b"midiStreamStop\0",
    b"mixerGetControlDetailsW\0",
    b"mixerGetDevCapsA\0",
    b"mixerGetDevCapsW\0",
    b"mixerGetID\0",
    b"mixerGetLineControlsW\0",
    b"mixerGetLineInfoW\0",
    b"mixerMessage\0",
    b"mmGetCurrentTask\0",
    b"mmTaskBlock\0",
    b"mmTaskCreate\0",
    b"mmTaskSignal\0",
    b"mmTaskYield\0",
    b"mmioAdvance\0",
    b"mmioAscend\0",
    b"mmioClose\0",
    b"mmioCreateChunk\0",
    b"mmioDescend\0",
    b"mmioFlush\0",
    b"mmioGetInfo\0",
    b"mmioInstallIOProcA\0",
    b"mmioInstallIOProcW\0",
    b"mmioOpenA\0",
    b"mmioOpenW\0",
    b"mmioRead\0",
    b"mmioRenameA\0",
    b"mmioRenameW\0",
    b"mmioSeek\0",
    b"mmioSendMessage\0",
    b"mmioSetBuffer\0",
    b"mmioSetInfo\0",
    b"mmioStringToFOURCCA\0",
    b"mmioStringToFOURCCW\0",
    b"mmioWrite\0",
    b"mmsystemGetVersion\0",
    b"sndPlaySoundA\0",
    b"sndPlaySoundW\0",
    b"timeBeginPeriod\0",
    b"timeEndPeriod\0",
    b"timeGetDevCaps\0",
    b"timeGetSystemTime\0",
    b"timeKillEvent\0",
    b"timeSetEvent\0",
    b"waveInAddBuffer\0",
    b"waveInClose\0",
    b"waveInGetDevCapsA\0",
    b"waveInGetDevCapsW\0",
    b"waveInGetErrorTextA\0",
    b"waveInGetErrorTextW\0",
    b"waveInGetID\0",
    b"waveInGetNumDevs\0",
    b"waveInGetPosition\0",
    b"waveInMessage\0",
    b"waveInOpen\0",
    b"waveInPrepareHeader\0",
    b"waveInReset\0",
    b"waveInStart\0",
    b"waveInStop\0",
    b"waveInUnprepareHeader\0",
    b"waveOutBreakLoop\0",
    b"waveOutClose\0",
    b"waveOutGetDevCapsA\0",
    b"waveOutGetDevCapsW\0",
    b"waveOutGetErrorTextA\0",
    b"waveOutGetErrorTextW\0",
    b"waveOutGetID\0",
    b"waveOutGetNumDevs\0",
    b"waveOutGetPitch\0",
    b"waveOutGetPlaybackRate\0",
    b"waveOutGetPosition\0",
    b"waveOutGetVolume\0",
    b"waveOutMessage\0",
    b"waveOutOpen\0",
    b"waveOutPause\0",
    b"waveOutPrepareHeader\0",
    b"waveOutReset\0",
    b"waveOutRestart\0",
    b"waveOutSetPitch\0",
    b"waveOutSetPlaybackRate\0",
    b"waveOutSetVolume\0",
    b"waveOutUnprepareHeader\0",
    b"waveOutWrite\0",
];
static RESOLVED:[AtomicUsize;171]=[const {AtomicUsize::new(0)};171];
#[inline(never)]
extern "C" fn resolve_extra(index:u32)->usize{
    let Some((slot,name))=RESOLVED.get(index as usize).zip(EXTRA.get(index as usize))
    else {std::process::abort();};
    let old=slot.load(Ordering::Acquire);
    if old!=0{return old;}
    let address=crate::winmm_proxy::resolve_external(name);
    if address==0 {std::process::abort();}
    slot.store(address,Ordering::Release);
    address
}
global_asm!(r#"
.text

    .globl _crimson_winmm_extra_0
    .p2align 4, 0x90
_crimson_winmm_extra_0:
    pushal
    pushl $0
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_1
    .p2align 4, 0x90
_crimson_winmm_extra_1:
    pushal
    pushl $1
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_2
    .p2align 4, 0x90
_crimson_winmm_extra_2:
    pushal
    pushl $2
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_3
    .p2align 4, 0x90
_crimson_winmm_extra_3:
    pushal
    pushl $3
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_4
    .p2align 4, 0x90
_crimson_winmm_extra_4:
    pushal
    pushl $4
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_5
    .p2align 4, 0x90
_crimson_winmm_extra_5:
    pushal
    pushl $5
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_6
    .p2align 4, 0x90
_crimson_winmm_extra_6:
    pushal
    pushl $6
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_7
    .p2align 4, 0x90
_crimson_winmm_extra_7:
    pushal
    pushl $7
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_8
    .p2align 4, 0x90
_crimson_winmm_extra_8:
    pushal
    pushl $8
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_9
    .p2align 4, 0x90
_crimson_winmm_extra_9:
    pushal
    pushl $9
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_10
    .p2align 4, 0x90
_crimson_winmm_extra_10:
    pushal
    pushl $10
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_11
    .p2align 4, 0x90
_crimson_winmm_extra_11:
    pushal
    pushl $11
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_12
    .p2align 4, 0x90
_crimson_winmm_extra_12:
    pushal
    pushl $12
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_13
    .p2align 4, 0x90
_crimson_winmm_extra_13:
    pushal
    pushl $13
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_14
    .p2align 4, 0x90
_crimson_winmm_extra_14:
    pushal
    pushl $14
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_15
    .p2align 4, 0x90
_crimson_winmm_extra_15:
    pushal
    pushl $15
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_16
    .p2align 4, 0x90
_crimson_winmm_extra_16:
    pushal
    pushl $16
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_17
    .p2align 4, 0x90
_crimson_winmm_extra_17:
    pushal
    pushl $17
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_18
    .p2align 4, 0x90
_crimson_winmm_extra_18:
    pushal
    pushl $18
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_19
    .p2align 4, 0x90
_crimson_winmm_extra_19:
    pushal
    pushl $19
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_20
    .p2align 4, 0x90
_crimson_winmm_extra_20:
    pushal
    pushl $20
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_21
    .p2align 4, 0x90
_crimson_winmm_extra_21:
    pushal
    pushl $21
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_22
    .p2align 4, 0x90
_crimson_winmm_extra_22:
    pushal
    pushl $22
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_23
    .p2align 4, 0x90
_crimson_winmm_extra_23:
    pushal
    pushl $23
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_24
    .p2align 4, 0x90
_crimson_winmm_extra_24:
    pushal
    pushl $24
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_25
    .p2align 4, 0x90
_crimson_winmm_extra_25:
    pushal
    pushl $25
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_26
    .p2align 4, 0x90
_crimson_winmm_extra_26:
    pushal
    pushl $26
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_27
    .p2align 4, 0x90
_crimson_winmm_extra_27:
    pushal
    pushl $27
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_28
    .p2align 4, 0x90
_crimson_winmm_extra_28:
    pushal
    pushl $28
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_29
    .p2align 4, 0x90
_crimson_winmm_extra_29:
    pushal
    pushl $29
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_30
    .p2align 4, 0x90
_crimson_winmm_extra_30:
    pushal
    pushl $30
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_31
    .p2align 4, 0x90
_crimson_winmm_extra_31:
    pushal
    pushl $31
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_32
    .p2align 4, 0x90
_crimson_winmm_extra_32:
    pushal
    pushl $32
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_33
    .p2align 4, 0x90
_crimson_winmm_extra_33:
    pushal
    pushl $33
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_34
    .p2align 4, 0x90
_crimson_winmm_extra_34:
    pushal
    pushl $34
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_35
    .p2align 4, 0x90
_crimson_winmm_extra_35:
    pushal
    pushl $35
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_36
    .p2align 4, 0x90
_crimson_winmm_extra_36:
    pushal
    pushl $36
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_37
    .p2align 4, 0x90
_crimson_winmm_extra_37:
    pushal
    pushl $37
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_38
    .p2align 4, 0x90
_crimson_winmm_extra_38:
    pushal
    pushl $38
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_39
    .p2align 4, 0x90
_crimson_winmm_extra_39:
    pushal
    pushl $39
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_40
    .p2align 4, 0x90
_crimson_winmm_extra_40:
    pushal
    pushl $40
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_41
    .p2align 4, 0x90
_crimson_winmm_extra_41:
    pushal
    pushl $41
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_42
    .p2align 4, 0x90
_crimson_winmm_extra_42:
    pushal
    pushl $42
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_43
    .p2align 4, 0x90
_crimson_winmm_extra_43:
    pushal
    pushl $43
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_44
    .p2align 4, 0x90
_crimson_winmm_extra_44:
    pushal
    pushl $44
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_45
    .p2align 4, 0x90
_crimson_winmm_extra_45:
    pushal
    pushl $45
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_46
    .p2align 4, 0x90
_crimson_winmm_extra_46:
    pushal
    pushl $46
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_47
    .p2align 4, 0x90
_crimson_winmm_extra_47:
    pushal
    pushl $47
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_48
    .p2align 4, 0x90
_crimson_winmm_extra_48:
    pushal
    pushl $48
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_49
    .p2align 4, 0x90
_crimson_winmm_extra_49:
    pushal
    pushl $49
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_50
    .p2align 4, 0x90
_crimson_winmm_extra_50:
    pushal
    pushl $50
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_51
    .p2align 4, 0x90
_crimson_winmm_extra_51:
    pushal
    pushl $51
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_52
    .p2align 4, 0x90
_crimson_winmm_extra_52:
    pushal
    pushl $52
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_53
    .p2align 4, 0x90
_crimson_winmm_extra_53:
    pushal
    pushl $53
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_54
    .p2align 4, 0x90
_crimson_winmm_extra_54:
    pushal
    pushl $54
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_55
    .p2align 4, 0x90
_crimson_winmm_extra_55:
    pushal
    pushl $55
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_56
    .p2align 4, 0x90
_crimson_winmm_extra_56:
    pushal
    pushl $56
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_57
    .p2align 4, 0x90
_crimson_winmm_extra_57:
    pushal
    pushl $57
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_58
    .p2align 4, 0x90
_crimson_winmm_extra_58:
    pushal
    pushl $58
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_59
    .p2align 4, 0x90
_crimson_winmm_extra_59:
    pushal
    pushl $59
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_60
    .p2align 4, 0x90
_crimson_winmm_extra_60:
    pushal
    pushl $60
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_61
    .p2align 4, 0x90
_crimson_winmm_extra_61:
    pushal
    pushl $61
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_62
    .p2align 4, 0x90
_crimson_winmm_extra_62:
    pushal
    pushl $62
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_63
    .p2align 4, 0x90
_crimson_winmm_extra_63:
    pushal
    pushl $63
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_64
    .p2align 4, 0x90
_crimson_winmm_extra_64:
    pushal
    pushl $64
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_65
    .p2align 4, 0x90
_crimson_winmm_extra_65:
    pushal
    pushl $65
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_66
    .p2align 4, 0x90
_crimson_winmm_extra_66:
    pushal
    pushl $66
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_67
    .p2align 4, 0x90
_crimson_winmm_extra_67:
    pushal
    pushl $67
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_68
    .p2align 4, 0x90
_crimson_winmm_extra_68:
    pushal
    pushl $68
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_69
    .p2align 4, 0x90
_crimson_winmm_extra_69:
    pushal
    pushl $69
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_70
    .p2align 4, 0x90
_crimson_winmm_extra_70:
    pushal
    pushl $70
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_71
    .p2align 4, 0x90
_crimson_winmm_extra_71:
    pushal
    pushl $71
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_72
    .p2align 4, 0x90
_crimson_winmm_extra_72:
    pushal
    pushl $72
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_73
    .p2align 4, 0x90
_crimson_winmm_extra_73:
    pushal
    pushl $73
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_74
    .p2align 4, 0x90
_crimson_winmm_extra_74:
    pushal
    pushl $74
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_75
    .p2align 4, 0x90
_crimson_winmm_extra_75:
    pushal
    pushl $75
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_76
    .p2align 4, 0x90
_crimson_winmm_extra_76:
    pushal
    pushl $76
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_77
    .p2align 4, 0x90
_crimson_winmm_extra_77:
    pushal
    pushl $77
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_78
    .p2align 4, 0x90
_crimson_winmm_extra_78:
    pushal
    pushl $78
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_79
    .p2align 4, 0x90
_crimson_winmm_extra_79:
    pushal
    pushl $79
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_80
    .p2align 4, 0x90
_crimson_winmm_extra_80:
    pushal
    pushl $80
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_81
    .p2align 4, 0x90
_crimson_winmm_extra_81:
    pushal
    pushl $81
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_82
    .p2align 4, 0x90
_crimson_winmm_extra_82:
    pushal
    pushl $82
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_83
    .p2align 4, 0x90
_crimson_winmm_extra_83:
    pushal
    pushl $83
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_84
    .p2align 4, 0x90
_crimson_winmm_extra_84:
    pushal
    pushl $84
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_85
    .p2align 4, 0x90
_crimson_winmm_extra_85:
    pushal
    pushl $85
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_86
    .p2align 4, 0x90
_crimson_winmm_extra_86:
    pushal
    pushl $86
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_87
    .p2align 4, 0x90
_crimson_winmm_extra_87:
    pushal
    pushl $87
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_88
    .p2align 4, 0x90
_crimson_winmm_extra_88:
    pushal
    pushl $88
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_89
    .p2align 4, 0x90
_crimson_winmm_extra_89:
    pushal
    pushl $89
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_90
    .p2align 4, 0x90
_crimson_winmm_extra_90:
    pushal
    pushl $90
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_91
    .p2align 4, 0x90
_crimson_winmm_extra_91:
    pushal
    pushl $91
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_92
    .p2align 4, 0x90
_crimson_winmm_extra_92:
    pushal
    pushl $92
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_93
    .p2align 4, 0x90
_crimson_winmm_extra_93:
    pushal
    pushl $93
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_94
    .p2align 4, 0x90
_crimson_winmm_extra_94:
    pushal
    pushl $94
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_95
    .p2align 4, 0x90
_crimson_winmm_extra_95:
    pushal
    pushl $95
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_96
    .p2align 4, 0x90
_crimson_winmm_extra_96:
    pushal
    pushl $96
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_97
    .p2align 4, 0x90
_crimson_winmm_extra_97:
    pushal
    pushl $97
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_98
    .p2align 4, 0x90
_crimson_winmm_extra_98:
    pushal
    pushl $98
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_99
    .p2align 4, 0x90
_crimson_winmm_extra_99:
    pushal
    pushl $99
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_100
    .p2align 4, 0x90
_crimson_winmm_extra_100:
    pushal
    pushl $100
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_101
    .p2align 4, 0x90
_crimson_winmm_extra_101:
    pushal
    pushl $101
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_102
    .p2align 4, 0x90
_crimson_winmm_extra_102:
    pushal
    pushl $102
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_103
    .p2align 4, 0x90
_crimson_winmm_extra_103:
    pushal
    pushl $103
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_104
    .p2align 4, 0x90
_crimson_winmm_extra_104:
    pushal
    pushl $104
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_105
    .p2align 4, 0x90
_crimson_winmm_extra_105:
    pushal
    pushl $105
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_106
    .p2align 4, 0x90
_crimson_winmm_extra_106:
    pushal
    pushl $106
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_107
    .p2align 4, 0x90
_crimson_winmm_extra_107:
    pushal
    pushl $107
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_108
    .p2align 4, 0x90
_crimson_winmm_extra_108:
    pushal
    pushl $108
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_109
    .p2align 4, 0x90
_crimson_winmm_extra_109:
    pushal
    pushl $109
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_110
    .p2align 4, 0x90
_crimson_winmm_extra_110:
    pushal
    pushl $110
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_111
    .p2align 4, 0x90
_crimson_winmm_extra_111:
    pushal
    pushl $111
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_112
    .p2align 4, 0x90
_crimson_winmm_extra_112:
    pushal
    pushl $112
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_113
    .p2align 4, 0x90
_crimson_winmm_extra_113:
    pushal
    pushl $113
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_114
    .p2align 4, 0x90
_crimson_winmm_extra_114:
    pushal
    pushl $114
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_115
    .p2align 4, 0x90
_crimson_winmm_extra_115:
    pushal
    pushl $115
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_116
    .p2align 4, 0x90
_crimson_winmm_extra_116:
    pushal
    pushl $116
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_117
    .p2align 4, 0x90
_crimson_winmm_extra_117:
    pushal
    pushl $117
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_118
    .p2align 4, 0x90
_crimson_winmm_extra_118:
    pushal
    pushl $118
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_119
    .p2align 4, 0x90
_crimson_winmm_extra_119:
    pushal
    pushl $119
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_120
    .p2align 4, 0x90
_crimson_winmm_extra_120:
    pushal
    pushl $120
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_121
    .p2align 4, 0x90
_crimson_winmm_extra_121:
    pushal
    pushl $121
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_122
    .p2align 4, 0x90
_crimson_winmm_extra_122:
    pushal
    pushl $122
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_123
    .p2align 4, 0x90
_crimson_winmm_extra_123:
    pushal
    pushl $123
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_124
    .p2align 4, 0x90
_crimson_winmm_extra_124:
    pushal
    pushl $124
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_125
    .p2align 4, 0x90
_crimson_winmm_extra_125:
    pushal
    pushl $125
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_126
    .p2align 4, 0x90
_crimson_winmm_extra_126:
    pushal
    pushl $126
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_127
    .p2align 4, 0x90
_crimson_winmm_extra_127:
    pushal
    pushl $127
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_128
    .p2align 4, 0x90
_crimson_winmm_extra_128:
    pushal
    pushl $128
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_129
    .p2align 4, 0x90
_crimson_winmm_extra_129:
    pushal
    pushl $129
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_130
    .p2align 4, 0x90
_crimson_winmm_extra_130:
    pushal
    pushl $130
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_131
    .p2align 4, 0x90
_crimson_winmm_extra_131:
    pushal
    pushl $131
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_132
    .p2align 4, 0x90
_crimson_winmm_extra_132:
    pushal
    pushl $132
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_133
    .p2align 4, 0x90
_crimson_winmm_extra_133:
    pushal
    pushl $133
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_134
    .p2align 4, 0x90
_crimson_winmm_extra_134:
    pushal
    pushl $134
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_135
    .p2align 4, 0x90
_crimson_winmm_extra_135:
    pushal
    pushl $135
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_136
    .p2align 4, 0x90
_crimson_winmm_extra_136:
    pushal
    pushl $136
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_137
    .p2align 4, 0x90
_crimson_winmm_extra_137:
    pushal
    pushl $137
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_138
    .p2align 4, 0x90
_crimson_winmm_extra_138:
    pushal
    pushl $138
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_139
    .p2align 4, 0x90
_crimson_winmm_extra_139:
    pushal
    pushl $139
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_140
    .p2align 4, 0x90
_crimson_winmm_extra_140:
    pushal
    pushl $140
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_141
    .p2align 4, 0x90
_crimson_winmm_extra_141:
    pushal
    pushl $141
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_142
    .p2align 4, 0x90
_crimson_winmm_extra_142:
    pushal
    pushl $142
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_143
    .p2align 4, 0x90
_crimson_winmm_extra_143:
    pushal
    pushl $143
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_144
    .p2align 4, 0x90
_crimson_winmm_extra_144:
    pushal
    pushl $144
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_145
    .p2align 4, 0x90
_crimson_winmm_extra_145:
    pushal
    pushl $145
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_146
    .p2align 4, 0x90
_crimson_winmm_extra_146:
    pushal
    pushl $146
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_147
    .p2align 4, 0x90
_crimson_winmm_extra_147:
    pushal
    pushl $147
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_148
    .p2align 4, 0x90
_crimson_winmm_extra_148:
    pushal
    pushl $148
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_149
    .p2align 4, 0x90
_crimson_winmm_extra_149:
    pushal
    pushl $149
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_150
    .p2align 4, 0x90
_crimson_winmm_extra_150:
    pushal
    pushl $150
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_151
    .p2align 4, 0x90
_crimson_winmm_extra_151:
    pushal
    pushl $151
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_152
    .p2align 4, 0x90
_crimson_winmm_extra_152:
    pushal
    pushl $152
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_153
    .p2align 4, 0x90
_crimson_winmm_extra_153:
    pushal
    pushl $153
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_154
    .p2align 4, 0x90
_crimson_winmm_extra_154:
    pushal
    pushl $154
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_155
    .p2align 4, 0x90
_crimson_winmm_extra_155:
    pushal
    pushl $155
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_156
    .p2align 4, 0x90
_crimson_winmm_extra_156:
    pushal
    pushl $156
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_157
    .p2align 4, 0x90
_crimson_winmm_extra_157:
    pushal
    pushl $157
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_158
    .p2align 4, 0x90
_crimson_winmm_extra_158:
    pushal
    pushl $158
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_159
    .p2align 4, 0x90
_crimson_winmm_extra_159:
    pushal
    pushl $159
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_160
    .p2align 4, 0x90
_crimson_winmm_extra_160:
    pushal
    pushl $160
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_161
    .p2align 4, 0x90
_crimson_winmm_extra_161:
    pushal
    pushl $161
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_162
    .p2align 4, 0x90
_crimson_winmm_extra_162:
    pushal
    pushl $162
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_163
    .p2align 4, 0x90
_crimson_winmm_extra_163:
    pushal
    pushl $163
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_164
    .p2align 4, 0x90
_crimson_winmm_extra_164:
    pushal
    pushl $164
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_165
    .p2align 4, 0x90
_crimson_winmm_extra_165:
    pushal
    pushl $165
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_166
    .p2align 4, 0x90
_crimson_winmm_extra_166:
    pushal
    pushl $166
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_167
    .p2align 4, 0x90
_crimson_winmm_extra_167:
    pushal
    pushl $167
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_168
    .p2align 4, 0x90
_crimson_winmm_extra_168:
    pushal
    pushl $168
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_169
    .p2align 4, 0x90
_crimson_winmm_extra_169:
    pushal
    pushl $169
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_winmm_extra_170
    .p2align 4, 0x90
_crimson_winmm_extra_170:
    pushal
    pushl $170
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax
"#, resolve = sym resolve_extra, options(att_syntax));
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn all_extended_named_exports_unique(){
        let mut names=std::collections::HashSet::new();
        for name in EXTRA {
            assert_eq!(name.last(),Some(&0u8));
            assert!(names.insert(name));
        }
        assert_eq!(EXTRA.len(),171);
        assert!(names.contains(b"timeBeginPeriod\0".as_slice()));
        assert!(names.contains(b"waveOutOpen\0".as_slice()));
    }
}
