---- MODULE RecLive ----
\* An identity INSTANCE whose fairness action reads a recursive function,
\* as specifications/allocator's SchedulingAllocator does.
EXTENDS Naturals
VARIABLES x, done
Sched == INSTANCE RecLiveInner
Init == Sched!Init
Next == Sched!Next
Spec == Init /\ [][Next]_<<x, done>> /\ Sched!Fair
Prop == Sched!Prop
====
