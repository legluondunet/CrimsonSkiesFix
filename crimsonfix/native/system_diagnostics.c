/* Read-only diagnostics adapted from SpiderMan2k1Fix (GPL-3.0-or-later).
 * Original project: https://github.com/legluondunet/SpiderMan2k1Fix
 * One-shot diagnostics for CrimsonFix, disabled unless [Debug] Log=1.
 * Runs once on startup, outside DllMain and only when [Debug] Log=1.
 *
 * Deliberately excludes username, home paths, hostname, IP, monitor serial,
 * Steam ID, MAC address, raw env paths and raw registry keys. This Windows
 * DLL cannot truthfully identify Linux Mesa/AMDGPU/NVIDIA kernel drivers or
 * the Proton tool version unless the host explicitly exposes a hint.
 */
#define WIN32_LEAN_AND_MEAN
#define COBJMACROS
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <dinput.h>
#include <d3d9.h>
#include <xinput.h>
#include "system_diagnostics.h"

typedef void (*SpiderDiagLogFn)(const char *);
static SpiderDiagLogFn diag_log=NULL;

static void emit(const char *s) {
  if(diag_log && s)diag_log(s);
}
/* Never copy raw environment values or filenames to public crash logs.
 * Restrict output to one printable line without path separators. */
static void clean(char *dst,size_t cap,const char *src) {
  size_t i=0;
  if(!dst || !cap)return;
  if(!src || !*src){snprintf(dst,cap,"Unavailable");return;}
  while(*src && i+1<cap) {
    unsigned char c=(unsigned char)*src++;
    if((c>='A'&&c<='Z') || (c>='a'&&c<='z') ||
       (c>='0'&&c<='9') || c==' ' || c=='.' ||
       c=='_' || c=='-' || c=='+' || c==':' ||
       c=='(' || c==')' || c==',' || c=='#')
      dst[i++]=(char)c;
    else if(c>=32 && c!=127)
      dst[i++]='?';
    else break; /* Embedded newline or control cannot forge log entries. */
  }
  while(i && dst[i-1]==' ')i--;
  dst[i]=0;
  if(!i)snprintf(dst,cap,"Unavailable");
}
static int env_clean(const char *key,char *out,size_t cap) {
  char raw[192];
  DWORD n;
  if(!key || !out || !cap)return 0;
  out[0]=0;
  n=GetEnvironmentVariableA(key,raw,sizeof(raw));
  /* Never read truncated values, which might be private absolute paths. */
  if(!n || n>=sizeof(raw))return 0;
  /* Version and XDG labels must NEVER be a filesystem path or email.
   * Cleaning delimiters alone would leave recognizable home/user names. */
  if(strpbrk(raw,"/\\@\r\n\t"))return 0;
  clean(out,cap,raw);
  return strcmp(out,"Unavailable")!=0;
}
static int env_present(const char *key) {
  DWORD n=GetEnvironmentVariableA(key,NULL,0);
  return n!=0;
}
/* Steam may pass a PATH LIST of compatibility tools, not an official
 * Proton version API. Never print it. Extract only a recognizable *single*
 * Proton tool directory component, and keep it explicitly unverified.
 * Example: /.../steamapps/common/GE-Proton9-21/files -> GE-Proton9-21.
 * Never print user-selected directory names or raw full paths. */
static int proton_tool_label(char *out,size_t cap) {
  char paths[4096],candidate[90];
  DWORD n=GetEnvironmentVariableA("STEAM_COMPAT_TOOL_PATHS",
                                  paths,sizeof(paths));
  const char *start,*p;
  if(!out || !cap || !n || n>=sizeof(paths))return 0;
  for(start=p=paths;;p++) {
    char ch=*p;
    if(ch=='/' || ch=='\\' || ch==':' || ch==';' || ch==0) {
      size_t len=(size_t)(p-start);
      int recognized=0,valid=1;
      size_t i;
      if(len && len<sizeof(candidate)) {
        memcpy(candidate,start,len);
        candidate[len]=0;
        if(!_strnicmp(candidate,"GE-Proton",9) &&
           candidate[9]>='0' && candidate[9]<='9')recognized=1;
        else if(!_strnicmp(candidate,"Proton ",7) &&
                ((candidate[7]>='0' && candidate[7]<='9') ||
                 !_stricmp(candidate+7,"Experimental") ||
                 !_stricmp(candidate+7,"Hotfix")))recognized=1;
        else if(!_strnicmp(candidate,"Proton-",7) &&
                candidate[7]>='0' && candidate[7]<='9')recognized=1;
        if(recognized) {
          for(i=0;i<len;i++) {
            unsigned char v=(unsigned char)candidate[i];
            if(!((v>='A'&&v<='Z') || (v>='a'&&v<='z') ||
                 (v>='0'&&v<='9') || v==' ' || v=='.' ||
                 v=='-' || v=='_' || v=='+')){valid=0;break;}
          }
          if(valid) {
            clean(out,cap,candidate);
            return 1;
          }
        }
      }
      if(!ch)break;
      start=p+1;
    }
  }
  return 0;
}
static void log_os(void) {
  HMODULE ntdll=GetModuleHandleA("ntdll.dll");
  typedef const char *(__cdecl *WineVersionFn)(void);
  typedef void (__cdecl *WineHostFn)(const char **,const char **);
  typedef LONG (WINAPI *RtlVersionFn)(OSVERSIONINFOW *);
  WineVersionFn wine=(WineVersionFn)(ntdll?GetProcAddress(ntdll,"wine_get_version"):NULL);
  WineHostFn host=(WineHostFn)(ntdll?GetProcAddress(ntdll,"wine_get_host_version"):NULL);
  RtlVersionFn rtl=(RtlVersionFn)(ntdll?GetProcAddress(ntdll,"RtlGetVersion"):NULL);
  const char *host_name=NULL,*host_release=NULL;
  OSVERSIONINFOW version;
  char wv[100],hn[60],hr[100],line[300],proton[100];
  int under_wine=wine!=NULL;
  int steam_hint=env_present("STEAM_COMPAT_DATA_PATH") ||
                 env_present("STEAM_COMPAT_CLIENT_INSTALL_PATH") ||
                 env_present("STEAM_COMPAT_TOOL_PATHS") ||
                 env_present("PROTON_VERSION");
  emit(under_wine?"SYS-OS: runtime=Wine (Win32 compatibility layer)":
                  "SYS-OS: runtime=Windows native");
  memset(&version,0,sizeof(version));
  version.dwOSVersionInfoSize=sizeof(version);
  if(rtl && rtl(&version)==0) {
    const char *label="Windows NT (version from RtlGetVersion)";
    if(version.dwMajorVersion==10 && version.dwBuildNumber>=22000)
      label="Windows 11-compatible";
    else if(version.dwMajorVersion==10)label="Windows 10-compatible";
    else if(version.dwMajorVersion==6 && version.dwMinorVersion==3)
      label="Windows 8.1-compatible";
    else if(version.dwMajorVersion==6 && version.dwMinorVersion==1)
      label="Windows 7-compatible";
    snprintf(line,sizeof(line),
      "SYS-OS: Win32 version=%lu.%lu build=%lu (%s%s)",
      (unsigned long)version.dwMajorVersion,
      (unsigned long)version.dwMinorVersion,
      (unsigned long)version.dwBuildNumber,label,
      under_wine?"; emulated by Wine, NOT host OS":"");
    emit(line);
  } else emit("SYS-OS: Win32 version=Unavailable (RtlGetVersion)");
  if(under_wine) {
    clean(wv,sizeof(wv),wine());
    snprintf(line,sizeof(line),"SYS-WINE: version=%s",wv);
    emit(line);
    if(host)host(&host_name,&host_release);
    clean(hn,sizeof(hn),host_name);
    clean(hr,sizeof(hr),host_release);
    snprintf(line,sizeof(line),
      "SYS-HOST: OS=%s kernel=%s (reported by Wine)",
      hn,hr);
    emit(line);
    if(steam_hint) {
      if(env_clean("PROTON_VERSION",proton,sizeof(proton))) {
        snprintf(line,sizeof(line),
          "SYS-PROTON: Steam compatibility environment=yes version=%s (environment hint, unverified)",
          proton);
        emit(line);
      } else if(proton_tool_label(proton,sizeof(proton))) {
        snprintf(line,sizeof(line),
          "SYS-PROTON: Steam compatibility environment=yes tool=%s (directory name hint, NOT verified runtime version)",
          proton);
        emit(line);
      } else
        emit("SYS-PROTON: Steam compatibility environment=yes version=Unavailable (not exposed by Wine/Steam)");
    } else emit("SYS-PROTON: environment hint=not detected; version=Unavailable (cannot exclude Proton)");
  } else {
    emit("SYS-WINE: not detected");
    emit("SYS-PROTON: not detected (native Windows)");
    emit("SYS-HOST: OS=Windows");
  }
}
static void log_cpu_memory(void) {
  HKEY key;
  char brand[160],cpu[150],line[300];
  DWORD type=0,len=sizeof(brand);
  SYSTEM_INFO sys;
  MEMORYSTATUSEX mem;
  typedef BOOL (WINAPI *LogicalInfoFn)(PSYSTEM_LOGICAL_PROCESSOR_INFORMATION,PDWORD);
  HMODULE kernel=GetModuleHandleA("kernel32.dll");
  LogicalInfoFn query=(LogicalInfoFn)(kernel?
    GetProcAddress(kernel,"GetLogicalProcessorInformation"):NULL);
  DWORD bytes=0;
  unsigned physical=0;
  brand[0]=0;
  if(RegOpenKeyExA(HKEY_LOCAL_MACHINE,
       "HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0",0,KEY_READ,&key)==ERROR_SUCCESS) {
    if(RegQueryValueExA(key,"ProcessorNameString",NULL,&type,
                        (LPBYTE)brand,&len)!=ERROR_SUCCESS ||
       type!=REG_SZ || !len)brand[0]=0;
    else brand[sizeof(brand)-1]=0;
    RegCloseKey(key);
  }
  /* Windows commonly reports "w/ Radeon" in processor descriptions.
   * Render that abbreviation without making the generic log sanitizer
   * accept slashes (which could otherwise expose private file paths). */
  {
    char normalized[192];
    const char *abbrev=strstr(brand," w/ ");
    if(abbrev) {
      size_t prefix=(size_t)(abbrev-brand);
      snprintf(normalized,sizeof(normalized),"%.*s with %s",
               (int)prefix,brand,abbrev+4);
      clean(cpu,sizeof(cpu),normalized);
    } else clean(cpu,sizeof(cpu),brand);
  }
  snprintf(line,sizeof(line),"SYS-CPU: model=%s",cpu);
  emit(line);
  GetSystemInfo(&sys);
  if(query && !query(NULL,&bytes) &&
     GetLastError()==ERROR_INSUFFICIENT_BUFFER &&
     bytes>=sizeof(SYSTEM_LOGICAL_PROCESSOR_INFORMATION) &&
     bytes<=1024u*1024u) {
    SYSTEM_LOGICAL_PROCESSOR_INFORMATION *info=
      (SYSTEM_LOGICAL_PROCESSOR_INFORMATION *)malloc(bytes);
    if(info) {
      if(query(info,&bytes)) {
        DWORD i,n=bytes/sizeof(*info);
        for(i=0;i<n;i++)
          if(info[i].Relationship==RelationProcessorCore)physical++;
      }
      free(info);
    }
  }
  snprintf(line,sizeof(line),
    "SYS-CPU: physical_cores=%s logical_processors=%lu (Win32 view)",
    physical?"detected":"Unavailable",(unsigned long)sys.dwNumberOfProcessors);
  emit(line);
  if(physical) {
    snprintf(line,sizeof(line),"SYS-CPU: physical_core_count=%u",physical);
    emit(line);
  }
  memset(&mem,0,sizeof(mem));
  mem.dwLength=sizeof(mem);
  if(GlobalMemoryStatusEx(&mem)) {
    snprintf(line,sizeof(line),
      "SYS-RAM: total=%llu MiB available=%llu MiB (Win32-reported; may differ from Linux host)",
      (unsigned long long)(mem.ullTotalPhys/1048576u),
      (unsigned long long)(mem.ullAvailPhys/1048576u));
    emit(line);
  } else emit("SYS-RAM: Unavailable (GlobalMemoryStatusEx)");
}
static void log_desktop(void) {
  char desktop[110],session[90],line[300];
  if(env_clean("XDG_CURRENT_DESKTOP",desktop,sizeof(desktop))) {
    snprintf(line,sizeof(line),"SYS-DESKTOP: DE=%s (XDG_CURRENT_DESKTOP hint)",desktop);
    emit(line);
  } else
    emit("SYS-DESKTOP: DE=Unavailable (Windows or Wine not exposing XDG_CURRENT_DESKTOP)");
  if(env_clean("XDG_SESSION_TYPE",session,sizeof(session))) {
    snprintf(line,sizeof(line),"SYS-DESKTOP: session=%s (XDG_SESSION_TYPE hint)",session);
    emit(line);
  }
}
static void log_screens(void) {
  unsigned i,active=0;
  char line[320],name[160],device[48];
  DISPLAY_DEVICEA d;
  DEVMODEA mode;
  snprintf(line,sizeof(line),"SYS-DISPLAYS: active_monitors=%d primary_desktop=%dx%d virtual_desktop=%dx%d",
    GetSystemMetrics(SM_CMONITORS),
    GetSystemMetrics(SM_CXSCREEN),GetSystemMetrics(SM_CYSCREEN),
    GetSystemMetrics(SM_CXVIRTUALSCREEN),GetSystemMetrics(SM_CYVIRTUALSCREEN));
  emit(line);
  /* Limit discovery, never log display registry paths, EDID, serials or
   * monitor instance GUIDs. DISPLAY_DEVICE indices start at zero. */
  for(i=0;i<16 && active<8;i++) {
    memset(&d,0,sizeof(d));d.cb=sizeof(d);
    if(!EnumDisplayDevicesA(NULL,i,&d,0))break;
    if(!(d.StateFlags&DISPLAY_DEVICE_ATTACHED_TO_DESKTOP))continue;
    active++;
    memset(&mode,0,sizeof(mode));mode.dmSize=sizeof(mode);
    clean(name,sizeof(name),d.DeviceString);
    {
      const char *suffix=strrchr(d.DeviceName,'\\');
      clean(device,sizeof(device),suffix?suffix+1:d.DeviceName);
    }
    if(EnumDisplaySettingsA(d.DeviceName,ENUM_CURRENT_SETTINGS,&mode)) {
      snprintf(line,sizeof(line),
        "SYS-DISPLAY: screen=%u device=%s primary=%s resolution=%lux%lu refresh=%luHz adapter=%s",
        active,device,(d.StateFlags&DISPLAY_DEVICE_PRIMARY_DEVICE)?"yes":"no",
        (unsigned long)mode.dmPelsWidth,(unsigned long)mode.dmPelsHeight,
        (unsigned long)mode.dmDisplayFrequency,name);
      emit(line);
    } else {
      snprintf(line,sizeof(line),
        "SYS-DISPLAY: screen=%u device=%s primary=%s resolution=Unavailable adapter=%s",
        active,device,(d.StateFlags&DISPLAY_DEVICE_PRIMARY_DEVICE)?"yes":"no",name);
      emit(line);
    }
  }
  if(!active)emit("SYS-DISPLAY: no active GDI display adapter enumerated");
}
static void log_graphics(void) {
  typedef IDirect3D9 *(WINAPI *CreateD3D9Fn)(UINT);
  HMODULE dll=LoadLibraryA("d3d9.dll");
  CreateD3D9Fn create=(CreateD3D9Fn)(dll?GetProcAddress(dll,"Direct3DCreate9"):NULL);
  IDirect3D9 *d3d=NULL;
  char line[310],desc[175],driver[125];
  unsigned i,n;
  if(create)d3d=create(D3D_SDK_VERSION);
  if(!d3d) {
    emit("SYS-GPU: D3D9 adapter=Unavailable (Direct3DCreate9)");
    emit("SYS-GRAPHICS-DRIVER: Unavailable (Windows API does not expose host Linux Mesa/AMDGPU/NVIDIA version)");
    if(dll)FreeLibrary(dll);
    return;
  }
  n=IDirect3D9_GetAdapterCount(d3d);
  for(i=0;i<n && i<8;i++) {
    D3DADAPTER_IDENTIFIER9 a;
    if(FAILED(IDirect3D9_GetAdapterIdentifier(d3d,i,0,&a)))continue;
    clean(desc,sizeof(desc),a.Description);
    clean(driver,sizeof(driver),a.Driver);
    snprintf(line,sizeof(line),
      "SYS-GPU: D3D9 adapter=%u model=%s vendor=0x%04lX device=0x%04lX",
      i,desc,(unsigned long)a.VendorId,(unsigned long)a.DeviceId);
    emit(line);
    snprintf(line,sizeof(line),
      "SYS-GRAPHICS-DRIVER: D3D9 adapter=%u reported_module=%s reported_version=%u.%u.%u.%u (may be Wine/DXVK wrapper, NOT Linux kernel/Mesa driver)",
      i,driver,
      (unsigned)HIWORD(a.DriverVersion.HighPart),
      (unsigned)LOWORD(a.DriverVersion.HighPart),
      (unsigned)HIWORD(a.DriverVersion.LowPart),
      (unsigned)LOWORD(a.DriverVersion.LowPart));
    emit(line);
  }
  if(!n)emit("SYS-GPU: D3D9 reports zero adapters");
  IDirect3D9_Release(d3d);
  FreeLibrary(dll);
}
typedef struct { unsigned total,logged; } SpiderDiagPads;
static BOOL CALLBACK diag_dinput_device(const DIDEVICEINSTANCEA *p,VOID *ctx) {
  SpiderDiagPads *pads=(SpiderDiagPads *)ctx;
  char line[270],name[165];
  unsigned vid,pid;
  if(!p || !pads)return DIENUM_STOP;
  pads->total++;
  if(pads->logged>=12)return DIENUM_STOP;
  vid=(unsigned)LOWORD(p->guidProduct.Data1);
  pid=(unsigned)HIWORD(p->guidProduct.Data1);
  clean(name,sizeof(name),p->tszProductName);
  snprintf(line,sizeof(line),
    "SYS-CONTROLLER: API=DirectInput8 slot=%u model=%s VID:PID=%04X:%04X (enumeration only, not acquired)",
    pads->logged+1,name,vid,pid);
  emit(line);
  pads->logged++;
  return DIENUM_CONTINUE;
}
static void log_controllers(void) {
  static const char *const libs[]={"xinput1_4.dll","xinput1_3.dll","xinput9_1_0.dll"};
  typedef DWORD (WINAPI *CapabilitiesFn)(DWORD,DWORD,XINPUT_CAPABILITIES *);
  typedef HRESULT (WINAPI *DirectInputCreateFn)(HINSTANCE,DWORD,REFIID,LPVOID *,LPUNKNOWN);
  unsigned i,xcount=0;
  HMODULE dll=NULL,di_module=NULL;
  CapabilitiesFn caps=NULL;
  DirectInputCreateFn create;
  IDirectInput8A *di=NULL;
  SpiderDiagPads pads={0,0};
  char line[240];
  for(i=0;i<sizeof(libs)/sizeof(libs[0]);i++) {
    dll=LoadLibraryA(libs[i]);
    caps=dll?(CapabilitiesFn)GetProcAddress(dll,"XInputGetCapabilities"):NULL;
    if(caps)break;
    if(dll)FreeLibrary(dll);
    dll=NULL;
  }
  if(caps) {
    for(i=0;i<XUSER_MAX_COUNT;i++) {
      XINPUT_CAPABILITIES c;
      memset(&c,0,sizeof(c));
      if(caps(i,0,&c)==ERROR_SUCCESS) {
        snprintf(line,sizeof(line),
          "SYS-CONTROLLER: API=XInput slot=%u subtype=%u model=Unavailable (XInput does not supply product name)",
          i,(unsigned)c.SubType);
        emit(line);xcount++;
      }
    }
  }
  if(dll)FreeLibrary(dll);
  di_module=LoadLibraryA("dinput8.dll");
  create=di_module?(DirectInputCreateFn)GetProcAddress(di_module,"DirectInput8Create"):NULL;
  if(create && SUCCEEDED(create(GetModuleHandleA(NULL),DIRECTINPUT_VERSION,
                                 &IID_IDirectInput8A,(void **)&di,NULL)) && di) {
    HRESULT hr=IDirectInput8_EnumDevices(di,DI8DEVCLASS_GAMECTRL,
                         diag_dinput_device,&pads,DIEDFL_ATTACHEDONLY);
    if(FAILED(hr))emit("SYS-CONTROLLER: DirectInput8 enumeration unavailable");
    IDirectInput8_Release(di);
  } else emit("SYS-CONTROLLER: DirectInput8 enumeration unavailable");
  if(di_module)FreeLibrary(di_module);
  snprintf(line,sizeof(line),
    "SYS-CONTROLLER: totals XInput=%u DirectInput8=%u (an XInput pad may also appear in DirectInput; do not add counts)",
    xcount,pads.total);
  emit(line);
}
void CrimsonSystemDiagnostics_Log(void (*logger)(const char *)) {
  DWORD tick_start,elapsed_ms;
  LARGE_INTEGER frequency,start,stop;
  int high_resolution;
  char line[128];
  if(!logger)return;
  /* Take timestamps only for this one-shot inventory, not per-frame.
   * QueryPerformanceCounter is precise on Windows and Wine; GetTickCount
   * remains a safe fallback when the high-resolution clock is unavailable. */
  tick_start=GetTickCount();
  high_resolution=QueryPerformanceFrequency(&frequency) &&
                  frequency.QuadPart>0 && QueryPerformanceCounter(&start);
  diag_log=logger;
  emit("SYS-INFO: BEGIN (passive startup inventory; diagnostics only; no personal identifiers)");
  log_os();
  log_cpu_memory();
  log_screens();
  log_graphics();
  log_desktop();
  log_controllers();
  elapsed_ms=GetTickCount()-tick_start; /* DWORD subtraction handles wraparound. */
  if(high_resolution && QueryPerformanceCounter(&stop) &&
     stop.QuadPart>=start.QuadPart) {
    ULONGLONG delta=(ULONGLONG)(stop.QuadPart-start.QuadPart);
    ULONGLONG hz=(ULONGLONG)frequency.QuadPart;
    ULONGLONG millis=(delta/hz)*1000ULL+((delta%hz)*1000ULL)/hz;
    elapsed_ms=(DWORD)(millis>0xFFFFFFFFULL?0xFFFFFFFFULL:millis);
  }
  snprintf(line,sizeof(line),
    "SYS-INFO: END elapsed_ms=%lu (unknown means unreported, not absent)",
    (unsigned long)elapsed_ms);
  emit(line);
  diag_log=NULL;
}
