// iOS 12 nema symbol kSecUseDataProtectionKeychain (az iOS 13). Rust knihovna
// security-framework na nej odkazuje, takze by aplikace na iOS 12 spadla hned
// pri startu (dyld: Symbol not found). Definujeme ho proto primo v aplikaci.
#include <CoreFoundation/CoreFoundation.h>
const CFStringRef kSecUseDataProtectionKeychain = CFSTR("nleg");
