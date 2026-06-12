import java.util.HashSet;
import java.util.Set;

public class Tasks {
    static long fact(long n) {
        return n < 2 ? 1 : n * fact(n - 1);
    }

    static long sumto(long n) {
        long t = 0;
        for (long i = 1; i <= n; i++) t += i;
        return t;
    }

    static long fib(long n) {
        return n < 2 ? n : fib(n - 1) + fib(n - 2);
    }

    static int distinct() {
        String[] words = {"the", "quick", "brown", "the", "lazy", "the", "fox"};
        Set<String> seen = new HashSet<>();
        for (String w : words) seen.add(w);
        return seen.size();
    }

    static long collatz(long n) {
        long x = n, s = 0;
        while (x != 1) {
            x = (x % 2 == 0) ? x / 2 : 3 * x + 1;
            s++;
        }
        return s;
    }

    public static void main(String[] args) {
        System.out.println(fact(12));
        System.out.println(sumto(100));
        System.out.println(fib(25));
        System.out.println(distinct());
        System.out.println(collatz(27));
    }
}
