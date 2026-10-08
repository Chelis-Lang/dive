---- MODULE LocalNotExported ----
---- MODULE A3 ----
LOCAL x == 1
====
---- MODULE C3 ----
EXTENDS A3
y == x
====
====
