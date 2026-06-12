package main

import "fmt"

func fact(n int) int {
	if n < 2 {
		return 1
	}
	return n * fact(n-1)
}

func sumto(n int) int {
	t := 0
	for i := 1; i <= n; i++ {
		t += i
	}
	return t
}

func fib(n int) int {
	if n < 2 {
		return n
	}
	return fib(n-1) + fib(n-2)
}

func distinct() int {
	words := []string{"the", "quick", "brown", "the", "lazy", "the", "fox"}
	seen := map[string]bool{}
	for _, w := range words {
		seen[w] = true
	}
	return len(seen)
}

func collatz(n int) int {
	x, s := n, 0
	for x != 1 {
		if x%2 == 0 {
			x = x / 2
		} else {
			x = 3*x + 1
		}
		s++
	}
	return s
}

func main() {
	fmt.Println(fact(12))
	fmt.Println(sumto(100))
	fmt.Println(fib(25))
	fmt.Println(distinct())
	fmt.Println(collatz(27))
}
