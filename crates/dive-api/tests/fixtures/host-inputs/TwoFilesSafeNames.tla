---- MODULE TwoFilesSafeNames ----
EXTENDS IOUtils
ReadOne == Deserialize("input.txt", [format |-> "TXT", charset |-> "UTF-8"])
ReadTwo == Deserialize("second.txt", [format |-> "TXT", charset |-> "UTF-8"])
Value == <<ReadOne.stdout, ReadTwo.stdout>>
Expected == <<"α-line\nβ-line\r\n", "second">>
VARIABLE x
Init == x = Value
Next == UNCHANGED x
Inv == x # Expected
====
