---- MODULE InstanceLocalHides ----
---- MODULE P ----
z == 1
====
LOCAL INSTANCE P
---- MODULE Q ----
EXTENDS LocalHidesOuterPlaceholder
====
====
