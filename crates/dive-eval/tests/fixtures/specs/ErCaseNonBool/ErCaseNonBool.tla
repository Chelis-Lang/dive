---- MODULE ErCaseNonBool ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME CASE 1 -> TRUE [] OTHER -> FALSE
====
