<?php
declare(strict_types=1);

namespace Billing;

final readonly class Order
{
    public function __construct(
        public string $customer,
        public int $cents,
        public bool $completed = true,
    ) {}
}

/** @param list<Order> $orders @return array<string, int> */
function summarize(array $orders): array
{
    $totals = [];
    foreach ($orders as $order) {
        if ($order->completed && $order->cents >= 0) {
            $totals[$order->customer] = ($totals[$order->customer] ?? 0) + $order->cents;
        }
    }
    arsort($totals, SORT_NUMERIC);
    return $totals;
}

$orders = [new Order('Ada', 1995), new Order('Zoë', 1250), new Order('Ada', 500, false)];
foreach (summarize($orders) as $customer => $cents) {
    $name = htmlspecialchars($customer, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8');
    echo <<<HTML
<total customer="$name">$cents cents</total>
HTML;
}
