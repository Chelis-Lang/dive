---- MODULE Context_P_no_pc ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F(a) == a + 1
 end define;
begin L: while TRUE do x := F(1); end while; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "98e1edb4" /\ chksum(tla) = "11b118c4")
VARIABLES x, y

(* define statement *)
F(a) == a + 1


vars == << x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0

Next == /\ x' = F(1)
        /\ y' = y

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
