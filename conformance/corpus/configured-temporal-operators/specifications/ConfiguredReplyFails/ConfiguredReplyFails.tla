---- MODULE ConfiguredReplyFails ----
CONSTANT Reply(_, _, _, _)
VARIABLE x
MCReply(p, d, old, new) == new = d
Init == x = 0
Respond == Reply(0, 1, x, x')
Spec == Init /\ [][Respond]_x /\ WF_x(Respond)
Live == []<>(x = 0)
====
