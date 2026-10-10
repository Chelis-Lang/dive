---- MODULE Recursive_P_reserved_wrapper ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Next(a=0)

begin
F:call Next(a); R:return;
end procedure;
begin
L:call Next(2); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "39d292ec" /\ chksum(tla) = "aab61d8")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Next *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ a' = a
        /\ stack' = << [ procedure |->  "Next",
                         pc        |->  "R",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "F"
     /\ x' = x

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Next == F \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Next",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Next \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
