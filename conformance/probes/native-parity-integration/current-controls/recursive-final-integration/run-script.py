import runpy,sys
from pathlib import Path
from argparse import Namespace
source,base,root,kind,target,native,java,jar=sys.argv[1:]
n=runpy.run_path(source,run_name="captured_controls")
f=n["main"];g=f.__globals__;g["BASE"]=Path(base);g["ROOT"]=Path(root)
a=Namespace(target=Path(target),native=Path(native),java=Path(java),jar=Path(jar))
f(a if kind!="guide-cli" else Path(native))
