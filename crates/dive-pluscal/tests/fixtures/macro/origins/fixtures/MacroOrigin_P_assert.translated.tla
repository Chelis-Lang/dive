---- MODULE MacroOrigin_P_assert ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin assert a>0; v:=a; end macro; macro Outer(v,a) begin Put(v,a); end macro; begin Outer(x, 1); skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4d4f619b" /\ chksum(tla) = "7f074fb7")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ Assert(1>0, 
                   "Failure of assertion at line 3, column 60 of macro called at line 3, column 139.")
         /\ x' = 1
         /\ TRUE
         /\ pc' = "Done"
         /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
