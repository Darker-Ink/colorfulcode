;;; Streams and a tiny interpreter in R7RS Scheme.
(import (scheme base) (scheme write))

#| Block comments
   #| can nest |# here. |#

(define pi 3.14159)

(define (square x) (* x x))

(define (fact n)
  (if (<= n 1)
      1
      (* n (fact (- n 1)))))

(define-record-type point
  (make-point x y)
  point?
  (x point-x)
  (y point-y set-point-y!))

(define (distance p q)
  (sqrt (+ (square (- (point-x p) (point-x q)))
           (square (- (point-y p) (point-y q))))))

(define (count-matching pred lst)
  (let loop ((lst lst) (acc 0))
    (cond ((null? lst) acc)
          ((pred (car lst)) (loop (cdr lst) (+ acc 1)))
          (else (loop (cdr lst) acc)))))

(define-syntax swap!
  (syntax-rules ()
    ((_ a b) (let ((tmp a)) (set! a b) (set! b tmp)))))

(define chars (list #\a #\space #\newline #\x41))
(define flags (vector #t #f #true #false))
(define numbers '(1 -2 3.5 1/3 #xFF #b1010 -0.5e3))

#;(display "this datum is commented out")

(let ((p (make-point 0 0))
      (q (make-point 3 4)))
  (display "distance: ")
  (display (distance p q))
  (newline)
  (display (count-matching even? `(1 2 ,(fact 3) 4)))
  (display "tab\there\n"))
