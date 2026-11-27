#include <stdio.h>
#include <string.h>
#include <stdlib.h>

int fib(int n) {
    if (n < 2) return n;
    return fib(n - 1) + fib(n - 2);
}

int soma(int *v, int n) {
    int s = 0;
    for (int i = 0; i < n; i++) s += v[i];
    return s;
}

const char *dia(int d) {
    switch (d) {
    case 0: return "domingo";
    case 1: return "segunda";
    case 2: return "terca";
    case 3: return "quarta";
    case 4: return "quinta";
    case 5: return "sexta";
    case 6: return "sabado";
    default: return "invalido";
    }
}

int conta(const char *s, char c) {
    int n = 0;
    while (*s) {
        if (*s == c) n++;
        s++;
    }
    return n;
}

int main(int argc, char **argv) {
    int v[5] = {1, 2, 3, 4, 5};
    printf("fib=%d soma=%d\n", fib(10), soma(v, 5));
    for (int i = 0; i < 7; i++) puts(dia(i));
    if (argc > 1) {
        printf("%d\n", conta(argv[1], 'a'));
    } else {
        puts("sem argumento");
    }
    return 0;
}
