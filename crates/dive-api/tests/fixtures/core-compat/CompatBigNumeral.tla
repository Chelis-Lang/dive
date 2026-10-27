---- MODULE CompatBigNumeral ----
\* TLC rejects a numeral outside 32 bits before checking starts
\* (TLC_INTEGER_TOO_BIG); the default profile has big integers (D-004).
EXTENDS Naturals
VARIABLE x
Init == x = 2147483648
Next == x' = x
====
