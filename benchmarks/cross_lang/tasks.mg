f fact(n){ if n < 2 { 1 } else { n * fact(n - 1) } }
f sumto(n){ var t = 0
 var i = 1
 while i <= n { t += i
 i += 1 }
 t }
f fib(n){ if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }
f distinct(){ len(keys(freq(["the", "quick", "brown", "the", "lazy", "the", "fox"]))) }
f collatz(n){ var x = n
 var steps = 0
 while x != 1 { if x % 2 == 0 { x = x / 2 } else { x = 3 * x + 1 }
 steps += 1 }
 steps }
