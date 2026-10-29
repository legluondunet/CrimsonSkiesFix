#ifndef CRIMSONFIX_SYSTEM_DIAGNOSTICS_H
#define CRIMSONFIX_SYSTEM_DIAGNOSTICS_H

/* Adapted from SpiderMan2k1Fix GPL-3.0-or-later.
 * Passive one-shot inventory, only when [Debug] Log=1; no DllMain calls. */
void CrimsonSystemDiagnostics_Log(void (*logger)(const char *));

#endif
