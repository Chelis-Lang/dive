---- MODULE CompatShape ----
\* TLC fails comparing a tuple with a record; the core refuses under
\* --compat=tlc rather than answer FALSE as D-009 does.
VARIABLE x
Init == x = <<1>>
Next == x' = IF x = [a |-> 1] THEN <<2>> ELSE x
====
