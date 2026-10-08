---- MODULE DefRecursiveInLet ----
x == LET RECURSIVE G(_)
         G(n) == n
     IN G(1)
====
