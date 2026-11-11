// iOS 12 nema symbol kSecUseDataProtectionKeychain (az iOS 13). Rust knihovna
// security-framework na nej odkazuje, takze by aplikace na iOS 12 spadla hned
// pri startu (dyld: Symbol not found). Definujeme ho proto primo v aplikaci.
#include <CoreFoundation/CoreFoundation.h>
const CFStringRef kSecUseDataProtectionKeychain = CFSTR("nleg");

// SecTrustCopyCertificateChain existuje az od iOS 15. Pouziva ji Go (crypto/x509
// uvnitr zabudovaneho Tailscale) pri overovani TLS certifikatu. Na iOS 12 ji
// nahradime starsim API se stejnym vysledkem (pole certifikatu v retezci).
typedef struct __SecTrust *SecTrustRef;
typedef struct __SecCertificate *SecCertificateRef;
extern CFIndex SecTrustGetCertificateCount(SecTrustRef trust);
extern SecCertificateRef SecTrustGetCertificateAtIndex(SecTrustRef trust, CFIndex ix);

CFArrayRef SecTrustCopyCertificateChain(SecTrustRef trust) {
    CFIndex n = SecTrustGetCertificateCount(trust);
    CFMutableArrayRef chain = CFArrayCreateMutable(kCFAllocatorDefault, n, &kCFTypeArrayCallBacks);
    for (CFIndex i = 0; i < n; i++) {
        SecCertificateRef cert = SecTrustGetCertificateAtIndex(trust, i);
        if (cert) CFArrayAppendValue(chain, cert);
    }
    return chain;
}
