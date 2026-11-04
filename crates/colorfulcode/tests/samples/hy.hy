;; A Hy script: Python semantics with Lisp syntax.
(import json
        os.path [join exists]
        collections [Counter])
(require hyrule [-> ->>])

(setv DEFAULT-PATH "data/words.json"
      threshold 3)

(defclass Inventory []
  "Keeps counts of items."

  (defn __init__ [self #* items]
    (setv self.counts (Counter items)))

  (defn add [self item [n 1]]
    (+= (get self.counts item) n)
    self)

  (defn common [self]
    (lfor [item n] (.most-common self.counts)
          :if (>= n threshold)
          item)))

(defn load-words [path]
  "Reads a JSON list of words, or returns an empty list."
  (if (exists path)
      (with [f (open path)]
        (json.load f))
      []))

(defmacro unless [test #* body]
  `(when (not ~test) ~@body))

(setv inv (Inventory "apple" "pear" "apple"))
(.add inv "pear" :n 4)

(unless (is inv None)
  (print f"common: {(.common inv)}\n")
  (print (len (load-words DEFAULT-PATH)) -1 2.5 0xFF True False))

(setv doc #[[raw "bracket" string \n]])
(print (->> [1 2 3] (map (fn [x] (* x x))) (list)))
