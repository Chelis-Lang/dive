---- MODULE SequentialModifier ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A { variable x=0; procedure Add(a=0) { Entry: + x:=x+a; Finish:return; }; { Start:call Add(1); Last:skip; } }
*)
\* BEGIN TRANSLATION (chksum(pcal) = "adeceacd" /\ chksum(tla) = "51cfb43")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Add *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "Start"

Entry == /\ pc = "Entry"
         /\ x' = x+a
         /\ pc' = "Finish"
         /\ UNCHANGED << stack, a >>

Finish == /\ pc = "Finish"
          /\ pc' = Head(stack).pc
          /\ a' = Head(stack).a
          /\ stack' = Tail(stack)
          /\ x' = x

Add == Entry \/ Finish

Start == /\ pc = "Start"
         /\ /\ a' = 1
            /\ stack' = << [ procedure |->  "Add",
                             pc        |->  "Last",
                             a         |->  a ] >>
                         \o stack
         /\ pc' = "Entry"
         /\ x' = x

Last == /\ pc = "Last"
        /\ TRUE
        /\ pc' = "Done"
        /\ UNCHANGED << x, stack, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Add \/ Start \/ Last
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
