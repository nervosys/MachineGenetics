f s(xs){ var acc = 0
 var out = [0]
 for x in xs { acc = acc + x
 out = out + [acc] }
 out }