---- MODULE ExecRevisit ----
VARIABLE x
Init == x = 0
Next == x' \in {0,1}
Live == [](x = 0 => <>(x = 1))
====
