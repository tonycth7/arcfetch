/* Link-time stubs for FreeBSD. The real libgcc_s.so.1, which ships with
 * FreeBSD, provides these symbols at run time; zig only bundles libc. */
void _Unwind_Resume(void) {}
void _Unwind_RaiseException(void) {}
void _Unwind_DeleteException(void) {}
void _Unwind_GetLanguageSpecificData(void) {}
void _Unwind_GetIP(void) {}
void _Unwind_GetIPInfo(void) {}
void _Unwind_SetGR(void) {}
void _Unwind_SetIP(void) {}
void _Unwind_GetRegionStart(void) {}
void _Unwind_GetTextRelBase(void) {}
void _Unwind_GetDataRelBase(void) {}
void _Unwind_FindEnclosingFunction(void) {}
void _Unwind_Backtrace(void) {}
void _Unwind_GetCFA(void) {}
void _Unwind_GetGR(void) {}
void _Unwind_GetBSP(void) {}
void _Unwind_Complete(void) {}
void _Unwind_GetAltTReg(void) {}
void _Unwind_IsSignalFrame(void) {}
