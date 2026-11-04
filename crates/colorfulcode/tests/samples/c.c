#include <stdio.h>
#include <stdlib.h>
#include "list.h"

#define MAX_ITEMS 128
#define SQUARE(x) ((x) * (x))

typedef struct node {
    int value;
    struct node *next;
} node_t;

static node_t *push(node_t *head, int value) {
    node_t *n = malloc(sizeof *n);
    if (n == NULL) {
        fprintf(stderr, "out of memory\n");
        exit(EXIT_FAILURE);
    }
    n->value = value;
    n->next = head;
    return n;
}

int main(int argc, char **argv) {
    node_t *list = NULL;
    for (int i = 0; i < MAX_ITEMS && i < argc; i++) {
        list = push(list, SQUARE(i));
    }
    /* Print and free the list. */
    while (list) {
        node_t *next = list->next;
        printf("%d '%c' %.2f\n", list->value, 'x', 3.14f);
        free(list);
        list = next;
    }
    return 0x0;
}
