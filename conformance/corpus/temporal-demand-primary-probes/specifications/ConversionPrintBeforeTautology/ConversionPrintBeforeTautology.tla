---- MODULE ConversionPrintBeforeTautology ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = 1 - x
Bad == IF x = 0 THEN 1 \div 0 = 0 ELSE TRUE
P == ([]<>Bad) => PrintT("CONVERSION_SEEN")
BindingMarker == PrintT("BIND_SEEN")
ASSUME PrintT("ASSUME_SEEN")
====
