#lang racket/base
;; A small inventory tracker.
(require racket/list
         racket/string
         (only-in racket/math pi))

(provide (struct-out item) total-value)

#| Nested #| block |# comments. |#

(struct item (name price qty) #:transparent)

(define inventory
  (list (item "widget" 2.50 10)
        (item "gadget" 12.99 3)
        (item "doohickey" 0.75 120)))

(define (total-value items)
  (for/sum ([it (in-list items)])
    (* (item-price it) (item-qty it))))

(define/contract (restock it n)
  (-> item? exact-nonnegative-integer? item?)
  (struct-copy item it [qty (+ (item-qty it) n)]))

(define (cheap? it #:limit [limit 1.0])
  (< (item-price it) limit))

(define (describe it)
  (match it
    [(item name price (? positive? qty))
     (format "~a: ~a @ $~a\n" name qty price)]
    [_ "out of stock"]))

(define pattern #rx"^[a-z]+$")
(define raw-bytes #"bytes\0")
(define flags (list #t #f #\a #\tab #xFF -42 1/2))

(module+ main
  (for ([it inventory])
    (display (describe it)))
  (printf "total: ~a\n" (total-value inventory))
  (displayln (string-join (map item-name (filter cheap? inventory)) ", "))
  (when (regexp-match? pattern "widget")
    (displayln (λ (x) (add1 x)))))
