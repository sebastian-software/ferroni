#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

#define MAX_ORDERS 128U
/* Store money in integer cents; never round each partial sum. */
typedef struct {
    const char *customer;
    int64_t cents;
    unsigned completed : 1;
} Order;

static int compare_order(const void *left, const void *right) {
    const Order *a = left, *b = right;
    return (b->cents > a->cents) - (b->cents < a->cents);
}

static int64_t total_for(const Order *orders, size_t count, const char *name) {
    int64_t total = 0;
    for (size_t i = 0; i < count; ++i) {
        if (orders[i].completed && strcmp(orders[i].customer, name) == 0)
            total += orders[i].cents;
    }
    return total;
}

int main(void) {
    Order orders[] = {{"Ada", 1995, 1}, {"Zoë", 1250, 1}, {"Ada", 500, 0}};
    const size_t count = sizeof orders / sizeof *orders;
    if (count > MAX_ORDERS) return EXIT_FAILURE;
    qsort(orders, count, sizeof *orders, compare_order);
    printf("<total> %lld cents\n", (long long)total_for(orders, count, "Ada"));
    return EXIT_SUCCESS;
}
