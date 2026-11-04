;;;; A tiny bank account system using CLOS.
(defpackage :bank
  (:use :cl)
  (:export #:account #:deposit #:withdraw))

(in-package :bank)

#| Block comments
   #| nest |# in Common Lisp. |#

(defparameter *interest-rate* 0.05
  "Yearly interest rate.")

(defconstant +max-balance+ 1000000)

(defclass account ()
  ((owner :initarg :owner :reader owner)
   (balance :initarg :balance :initform 0 :accessor balance)))

(define-condition insufficient-funds (error)
  ((needed :initarg :needed :reader needed)))

(defgeneric deposit (acct amount))

(defmethod deposit ((acct account) amount)
  (incf (balance acct) amount))

(defun withdraw (acct amount &key (force nil))
  "Removes AMOUNT from ACCT unless it would overdraw."
  (when (and (> amount (balance acct)) (not force))
    (error 'insufficient-funds :needed amount))
  (decf (balance acct) amount))

(defmacro with-account ((var owner) &body body)
  `(let ((,var (make-instance 'account :owner ,owner)))
     ,@body))

(defun apply-interest (accounts)
  (mapcar #'(lambda (a) (* (balance a) (1+ *interest-rate*))) accounts))

(with-account (acct "Ada")
  (deposit acct 100)
  (handler-case (withdraw acct 500)
    (insufficient-funds (c)
      (format t "Need ~a more~%" (needed c))))
  (format t "Balance: ~,2f~%" (balance acct))
  (print (list #\a #\Space #x1F -3/4 2.5d0 t nil))
  #+sbcl (print "running on SBCL"))
