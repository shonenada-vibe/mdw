## Plain Text

```
print 'hello world'
```

## JSON

```json
{
    "hello": "world"
}
```

## Rust

```rust
use std::collections::HashMap;

/// A generic repository for storing items by ID.
pub struct Repository<T> {
    items: HashMap<u64, T>,
    next_id: u64,
}

impl<T: Clone> Repository<T> {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn insert(&mut self, item: T) -> u64 {
        let id = self.next_id;
        self.items.insert(id, item);
        self.next_id += 1;
        id
    }

    pub fn get(&self, id: u64) -> Option<&T> {
        self.items.get(&id)
    }

    pub fn remove(&mut self, id: u64) -> Option<T> {
        self.items.remove(&id)
    }

    pub fn list(&self) -> Vec<(u64, &T)> {
        let mut entries: Vec<_> = self.items.iter().map(|(&k, v)| (k, v)).collect();
        entries.sort_by_key(|(k, _)| *k);
        entries
    }
}

fn main() {
    let mut repo = Repository::new();
    let id = repo.insert("Hello, world!".to_string());
    println!("Inserted with id={id}");

    if let Some(val) = repo.get(id) {
        println!("Found: {val}");
    }
}
```

## Python

```python
from dataclasses import dataclass, field
from typing import Optional
import asyncio
import aiohttp


@dataclass
class Config:
    base_url: str
    timeout: float = 10.0
    headers: dict[str, str] = field(default_factory=dict)


class ApiClient:
    """An async HTTP client with retry logic."""

    def __init__(self, config: Config):
        self._config = config
        self._session: Optional[aiohttp.ClientSession] = None

    async def __aenter__(self):
        self._session = aiohttp.ClientSession(
            headers=self._config.headers,
            timeout=aiohttp.ClientTimeout(total=self._config.timeout),
        )
        return self

    async def __aexit__(self, *exc):
        if self._session:
            await self._session.close()

    async def get(self, path: str, retries: int = 3) -> dict:
        url = f"{self._config.base_url}/{path.lstrip('/')}"
        for attempt in range(1, retries + 1):
            try:
                async with self._session.get(url) as resp:
                    resp.raise_for_status()
                    return await resp.json()
            except aiohttp.ClientError as e:
                if attempt == retries:
                    raise
                await asyncio.sleep(2 ** attempt)


async def main():
    config = Config(
        base_url="https://api.example.com",
        headers={"Authorization": "Bearer token123"},
    )
    async with ApiClient(config) as client:
        data = await client.get("/users")
        print(f"Fetched {len(data)} users")


if __name__ == "__main__":
    asyncio.run(main())
```

## JavaScript / TypeScript

```javascript
class EventEmitter {
  #listeners = new Map();

  on(event, callback) {
    if (!this.#listeners.has(event)) {
      this.#listeners.set(event, []);
    }
    this.#listeners.get(event).push(callback);
    return () => this.off(event, callback);
  }

  off(event, callback) {
    const cbs = this.#listeners.get(event);
    if (cbs) {
      this.#listeners.set(event, cbs.filter((cb) => cb !== callback));
    }
  }

  emit(event, ...args) {
    for (const cb of this.#listeners.get(event) ?? []) {
      cb(...args);
    }
  }
}

// Usage
const bus = new EventEmitter();
const unsub = bus.on("message", (msg) => console.log(`Received: ${msg}`));
bus.emit("message", "hello world");
unsub();
```

```typescript
interface Task {
  id: number;
  title: string;
  done: boolean;
}

function partition<T>(arr: T[], predicate: (item: T) => boolean): [T[], T[]] {
  const pass: T[] = [];
  const fail: T[] = [];
  for (const item of arr) {
    (predicate(item) ? pass : fail).push(item);
  }
  return [pass, fail];
}

const tasks: Task[] = [
  { id: 1, title: "Write docs", done: true },
  { id: 2, title: "Fix bug #42", done: false },
  { id: 3, title: "Code review", done: true },
  { id: 4, title: "Deploy v2", done: false },
];

const [completed, pending] = partition(tasks, (t) => t.done);
console.log("Completed:", completed.map((t) => t.title));
console.log("Pending:", pending.map((t) => t.title));
```

## Go

```go
package main

import (
	"fmt"
	"sync"
)

type SafeCounter struct {
	mu sync.RWMutex
	v  map[string]int
}

func NewSafeCounter() *SafeCounter {
	return &SafeCounter{v: make(map[string]int)}
}

func (c *SafeCounter) Inc(key string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.v[key]++
}

func (c *SafeCounter) Get(key string) int {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.v[key]
}

func main() {
	counter := NewSafeCounter()
	var wg sync.WaitGroup

	for i := 0; i < 1000; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			counter.Inc("hits")
		}()
	}

	wg.Wait()
	fmt.Printf("Total hits: %d\n", counter.Get("hits"))
}
```

## Shell

```bash
#!/usr/bin/env bash
set -euo pipefail

LOG_FILE="/tmp/deploy-$(date +%Y%m%d-%H%M%S).log"

log() {
    local level="$1"; shift
    printf "[%s] %s: %s\n" "$(date -Iseconds)" "$level" "$*" | tee -a "$LOG_FILE"
}

check_deps() {
    local missing=()
    for cmd in git docker curl jq; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        log ERROR "Missing dependencies: ${missing[*]}"
        exit 1
    fi
}

deploy() {
    local tag="${1:?Usage: deploy <tag>}"
    log INFO "Starting deployment of $tag"

    git fetch --tags
    git checkout "tags/$tag"
    docker build -t "myapp:$tag" .
    docker push "myapp:$tag"

    log INFO "Deployment of $tag complete"
}

check_deps
deploy "$@"
```

## SQL

```sql
WITH monthly_revenue AS (
    SELECT
        DATE_TRUNC('month', o.created_at) AS month,
        c.name                            AS customer,
        SUM(oi.quantity * oi.unit_price)   AS revenue
    FROM orders o
    JOIN order_items oi ON oi.order_id = o.id
    JOIN customers c    ON c.id = o.customer_id
    WHERE o.status = 'completed'
      AND o.created_at >= CURRENT_DATE - INTERVAL '12 months'
    GROUP BY 1, 2
),
ranked AS (
    SELECT
        month,
        customer,
        revenue,
        RANK() OVER (PARTITION BY month ORDER BY revenue DESC) AS rnk
    FROM monthly_revenue
)
SELECT month, customer, revenue
FROM ranked
WHERE rnk <= 5
ORDER BY month DESC, revenue DESC;
```

## C

```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct Node {
    int          data;
    struct Node *next;
} Node;

Node *list_push(Node *head, int value) {
    Node *n = malloc(sizeof(Node));
    if (!n) { perror("malloc"); exit(1); }
    n->data = value;
    n->next = head;
    return n;
}

Node *list_reverse(Node *head) {
    Node *prev = NULL, *curr = head, *next;
    while (curr) {
        next = curr->next;
        curr->next = prev;
        prev = curr;
        curr = next;
    }
    return prev;
}

void list_print(const Node *head) {
    for (const Node *n = head; n; n = n->next)
        printf("%d -> ", n->data);
    puts("NULL");
}

void list_free(Node *head) {
    while (head) {
        Node *tmp = head;
        head = head->next;
        free(tmp);
    }
}

int main(void) {
    Node *list = NULL;
    for (int i = 1; i <= 5; i++)
        list = list_push(list, i);

    printf("Original: ");
    list_print(list);

    list = list_reverse(list);
    printf("Reversed: ");
    list_print(list);

    list_free(list);
    return 0;
}
```

## Java

```java
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.stream.Collectors;

public sealed interface Shape permits Circle, Rectangle, Triangle {
    double area();
    String describe();
}

record Circle(double radius) implements Shape {
    @Override
    public double area() {
        return Math.PI * radius * radius;
    }

    @Override
    public String describe() {
        return "Circle(r=%.2f, area=%.2f)".formatted(radius, area());
    }
}

record Rectangle(double width, double height) implements Shape {
    @Override
    public double area() {
        return width * height;
    }

    @Override
    public String describe() {
        return "Rectangle(%s x %s, area=%.2f)".formatted(width, height, area());
    }
}

record Triangle(double base, double height) implements Shape {
    @Override
    public double area() {
        return 0.5 * base * height;
    }

    @Override
    public String describe() {
        return "Triangle(b=%.2f, h=%.2f, area=%.2f)".formatted(base, height, area());
    }
}

class ShapeAnalyzer {
    private final Map<String, List<Shape>> cache = new ConcurrentHashMap<>();

    public Map<String, Double> totalAreaByType(List<Shape> shapes) {
        return shapes.stream()
            .collect(Collectors.groupingBy(
                s -> s.getClass().getSimpleName(),
                Collectors.summingDouble(Shape::area)
            ));
    }

    public static void main(String[] args) {
        var shapes = List.of(
            new Circle(5.0),
            new Rectangle(3.0, 4.0),
            new Triangle(6.0, 8.0),
            new Circle(2.5),
            new Rectangle(10.0, 2.0)
        );

        var analyzer = new ShapeAnalyzer();
        shapes.forEach(s -> System.out.println(s.describe()));

        var totals = analyzer.totalAreaByType(shapes);
        totals.forEach((type, area) ->
            System.out.printf("Total %s area: %.2f%n", type, area));
    }
}
```

## PHP

```php
<?php

declare(strict_types=1);

namespace App\Service;

use App\Entity\Product;
use App\Repository\ProductRepository;
use Psr\Log\LoggerInterface;

readonly class PriceCalculator
{
    public function __construct(
        private ProductRepository $repository,
        private LoggerInterface $logger,
        private float $taxRate = 0.21,
    ) {}

    public function calculateTotal(array $items): PriceBreakdown
    {
        $subtotal = 0.0;

        foreach ($items as ['product_id' => $id, 'quantity' => $qty]) {
            $product = $this->repository->find($id)
                ?? throw new \InvalidArgumentException("Product {$id} not found");

            $subtotal += $product->getPrice() * $qty;

            $this->logger->debug('Item added', [
                'product' => $product->getName(),
                'unit_price' => $product->getPrice(),
                'quantity' => $qty,
            ]);
        }

        $discount = match (true) {
            $subtotal >= 500 => $subtotal * 0.15,
            $subtotal >= 200 => $subtotal * 0.10,
            $subtotal >= 100 => $subtotal * 0.05,
            default          => 0.0,
        };

        $taxable = $subtotal - $discount;
        $tax = $taxable * $this->taxRate;

        return new PriceBreakdown(
            subtotal: round($subtotal, 2),
            discount: round($discount, 2),
            tax: round($tax, 2),
            total: round($taxable + $tax, 2),
        );
    }

    public function formatReceipt(PriceBreakdown $breakdown): string
    {
        return <<<RECEIPT
        ================================
          Subtotal:  \${$breakdown->subtotal}
          Discount: -\${$breakdown->discount}
          Tax:       \${$breakdown->tax}
          --------------------------------
          Total:     \${$breakdown->total}
        ================================
        RECEIPT;
    }
}

readonly class PriceBreakdown
{
    public function __construct(
        public float $subtotal,
        public float $discount,
        public float $tax,
        public float $total,
    ) {}
}
```

## Clojure

```clojure
(ns app.pipeline
  (:require [clojure.string :as str]
            [clojure.core.async :as async :refer [<! >! go go-loop chan]]))

;; A transducer-based text processing pipeline

(defn normalize
  "Trim whitespace and lowercase a string."
  [s]
  (-> s str/trim str/lower-case))

(defn tokenize
  "Split text into words, removing punctuation."
  [text]
  (->> (str/split text #"\s+")
       (map #(str/replace % #"[^a-z0-9]" ""))
       (remove str/blank?)))

(def stop-words
  #{"the" "a" "an" "is" "are" "was" "were" "in" "on" "at"
    "to" "for" "of" "and" "or" "but" "not" "with" "this" "that"})

(defn word-frequencies
  "Count word frequencies in a collection of texts, ignoring stop words."
  [texts]
  (let [xf (comp
             (map normalize)
             (mapcat tokenize)
             (remove stop-words))]
    (->> (into [] xf texts)
         frequencies
         (sort-by val >))))

(defrecord Document [id title body tags])

(defprotocol Searchable
  (matches? [this query] "Check if document matches a search query.")
  (relevance [this query] "Compute relevance score for a query."))

(extend-type Document
  Searchable
  (matches? [doc query]
    (let [q (normalize query)
          text (normalize (str (:title doc) " " (:body doc)))]
      (str/includes? text q)))

  (relevance [doc query]
    (let [q (normalize query)
          tokens (tokenize (str (:title doc) " " (:body doc)))
          total (count tokens)
          hits (count (filter #(str/includes? % q) tokens))]
      (if (zero? total) 0.0 (double (/ hits total))))))

(defn search
  "Search documents and return results sorted by relevance."
  [documents query]
  (->> documents
       (filter #(matches? % query))
       (sort-by #(relevance % query) >)
       (map (fn [doc]
              {:id (:id doc)
               :title (:title doc)
               :score (relevance doc query)}))))

(defn async-process
  "Process items through a channel pipeline with backpressure."
  [items process-fn parallelism]
  (let [in-ch  (chan 100)
        out-ch (chan 100)]
    (dotimes [_ parallelism]
      (go-loop []
        (when-let [item (<! in-ch)]
          (let [result (process-fn item)]
            (>! out-ch result))
          (recur))))
    (go
      (doseq [item items]
        (>! in-ch item))
      (async/close! in-ch))
    out-ch))

;; Example usage
(comment
  (def docs
    [(->Document 1 "Clojure Basics" "Learn functional programming with Clojure" [:clojure :fp])
     (->Document 2 "Java Interop" "Calling Java libraries from Clojure code" [:clojure :java])
     (->Document 3 "Web Development" "Building REST APIs with Ring and Compojure" [:web :clojure])])

  (search docs "clojure")
  ;; => ({:id 1, :title "Clojure Basics", :score 0.166}
  ;;     {:id 2, :title "Java Interop",   :score 0.142}
  ;;     {:id 3, :title "Web Development", :score 0.142})

  (word-frequencies ["The quick brown fox jumps over the lazy dog"
                     "A quick red fox runs through the forest"])
  ;; => (["quick" 2] ["fox" 2] ["brown" 1] ["jumps" 1] ...)
  )
```

## YAML

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web-app
  labels:
    app: web
    version: v2
spec:
  replicas: 3
  selector:
    matchLabels:
      app: web
  template:
    metadata:
      labels:
        app: web
        version: v2
    spec:
      containers:
        - name: web
          image: myregistry/web-app:2.0.0
          ports:
            - containerPort: 8080
          env:
            - name: DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: db-credentials
                  key: url
          resources:
            requests:
              cpu: "100m"
              memory: "128Mi"
            limits:
              cpu: "500m"
              memory: "512Mi"
          livenessProbe:
            httpGet:
              path: /healthz
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 30
```
