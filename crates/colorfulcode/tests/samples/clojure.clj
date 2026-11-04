(ns my.app.core
  "Word counting with transducers."
  (:require [clojure.string :as str]
            [clojure.set :refer [union]])
  (:import (java.time Instant)))

;; Dynamic vars use earmuffs.
(def ^:dynamic *verbose* false)
(def max-words 1000)

(defprotocol Shape
  (area [this] "Returns the area."))

(defrecord Circle [r]
  Shape
  (area [_] (* Math/PI r r)))

(defn- tokenize
  "Splits text into lowercase words."
  [^String text]
  (->> (str/split text #"\s+")
       (map str/lower-case)
       (remove str/blank?)))

(defn word-counts [text]
  (let [words (tokenize text)]
    (when *verbose*
      (println "counting" (count words) "words\n"))
    (frequencies words)))

(defmulti describe :kind)
(defmethod describe :circle [{:keys [r]}]
  (format "circle r=%.2f" (double r)))

(def numbers [1 -2 3.5 22/7 0xFF 1e3 42N])
(def letters [\a \newline \space \u00e9 \é])
(def lookup {:a 1, ::local 2, :ns/qualified 3})

#_(this form is ignored entirely)

(defn -main [& args]
  (let [counts (word-counts "the cat and the hat")
        total (atom 0)]
    (doseq [[w n] (sort-by val > counts)]
      (swap! total + n)
      (println w n))
    (println (.toUpperCase "done") @total (inc (first numbers)))
    #?(:clj (Instant/now) :cljs (js/Date.))
    (mapv #(* % 2) (filter odd? (range 10)))
    (when (nil? args) nil)))
