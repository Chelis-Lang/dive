---- MODULE LvView ----
\* VIEW drops y, a counter that grows to 3 and stays. Liveness then runs on
\* view-equivalent states. <>(x = 9) is violated; where does TLC close the
\* loop, and what are the counts?
EXTENDS Naturals
VARIABLES x, y
vars == <<x, y>>
Init == x = 0 /\ y = 0
Next == x' = (x + 1) % 2 /\ y' = IF y < 3 THEN y + 1 ELSE y
Spec == Init /\ [][Next]_vars /\ WF_vars(Next)
Prop == <>(x = 9)
View == x
====
