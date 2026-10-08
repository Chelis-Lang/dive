---- MODULE CmTlcRegisters ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME PrintT(TLCSet(3, "v"))
ASSUME PrintT(TLCGet(3))
ASSUME PrintT(TLCSet(0, <<1>>))
ASSUME PrintT(TLCGet("all"))
ASSUME PrintT(TLCSet("s:k", 5))
ASSUME PrintT(TLCSet("s:a", 6))
ASSUME PrintT(TLCGet("s:k"))
ASSUME PrintT(TLCGet("all:named"))
ASSUME PrintT(TLCGet("level"))
ASSUME PrintT(TLCSet("exit", FALSE))
ASSUME PrintT(TLCSet("pause", FALSE))
====
