---- MODULE RecLiveBad ----
\* RecLive without fairness for Finish: <>[]done fails on a stuttering suffix.
EXTENDS Naturals
VARIABLES x, done
Sched == INSTANCE RecLiveInner
Spec == Sched!Init /\ [][Sched!Next]_<<x, done>> /\ WF_<<x, done>>(Sched!Step)
Prop == Sched!Prop
====
