---- MODULE ErrIntvSize ----
EXTENDS Integers, Sequences, FiniteSets, TLC, TLCExt
CONSTANTS mvq1, mvq2, T_q1, T_q2, U_q1, mv_q3
ASSUME PrintT(Cardinality(-2147483647..2147483647))
====
