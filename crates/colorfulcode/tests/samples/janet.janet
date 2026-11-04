# A small Janet program: parsing and tallying log lines.
(import spork/json :as json)

(def log-grammar
  ~{:level (<- (+ "INFO" "WARN" "ERROR"))
    :main (* :level ": " (<- (any 1)))})

(var total 0)

(defn parse-line
  "Parses one log line into a level and message."
  [line]
  (when-let [[level msg] (peg/match log-grammar line)]
    {:level (keyword level) :message msg}))

(defn- bump [counts level]
  (put counts level (+ 1 (get counts level 0))))

(defmacro with-timing [& body]
  ~(let [start (os/clock)]
     ,;body
     (- (os/clock) start)))

(def lines
  ["INFO: started" "WARN: disk 91%" "ERROR: crash\n" "INFO: done"])

(def long-text ``
A long string with "quotes"
and no escapes \n here.
``)

(defn main [& args]
  (def counts @{})
  (def elapsed
    (with-timing
      (each line lines
        (if-let [entry (parse-line line)]
          (do
            (bump counts (entry :level))
            (++ total))
          (eprint "unparsed: " line)))))
  (printf "%d lines in %.3fs" total elapsed)
  (pp counts)
  (print (string/format "%q" [1 -2 3.5 0xFF nil true false]))
  (print (length long-text)))
