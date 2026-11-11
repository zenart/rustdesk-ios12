#!/usr/bin/env python3
"""Upravi Go runtime pro stary cip Apple A7 (iPad mini 2, iOS 12).

Go 1.27 v runtime.procyieldAsm cte casovac instrukci `MRS CNTVCT_EL0`.
Na A7 je tato instrukce v uzivatelskem rezimu zakazana a aplikace spadne
(SIGILL) v okamziku, kdy Tailscale zacne pracovat. Funkci nahradime
jednoduchou smyckou bez cteni casovace (tak ji mely starsi verze Go).

Pouziti: patch_go_procyield.py <GOROOT>
"""
import re, sys

path = sys.argv[1] + "/src/runtime/asm_arm64.s"
src = open(path).read()
m = re.search(r"TEXT runtime·procyieldAsm\(SB\),NOSPLIT,\$0-0\n.*?\n\tRET\n", src, re.S)
if not m:
    sys.exit("procyieldAsm nenalezena - jina verze Go?")
if "CNTVCT_EL0" not in m.group(0):
    print("procyieldAsm uz casovac necte, neni co upravovat")
    sys.exit(0)
new = """TEXT runtime·procyieldAsm(SB),NOSPLIT,$0-0
	MOVWU	cycles+0(FP), R0
	CBZ	R0, done
	// A7/iOS 12: bez CNTVCT_EL0; zhruba 1 ISB na 16 ns pozadovaneho zpozdeni
	LSR	$4, R0, R0
	ADD	$1, R0, R0
again:
	ISB	$15
	SUB	$1, R0, R0
	CBNZ	R0, again
done:
	RET
"""
src = src[: m.start()] + new + src[m.end():]
if "CNTVCT_EL0" in src.replace("// ", ""):
    left = [l for l in src.splitlines() if "CNTVCT_EL0" in l and not l.strip().startswith("//")]
    if left:
        sys.exit("CNTVCT_EL0 zustava i jinde: %r" % left[:3])
open(path, "w").write(src)
print("procyieldAsm upravena:", path)
