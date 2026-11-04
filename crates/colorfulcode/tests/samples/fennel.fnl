;; A small game loop in Fennel for LOVE2D.
(local lume (require :lib.lume))

(var score 0)
(local speed 120.5)
(local player {:x 10 :y 20 :w 16 :h 16 :name "hero"})

(fn clamp [x lo hi]
  "Keeps x between lo and hi."
  (math.max lo (math.min hi x)))

(fn player.move [self dx dy dt]
  (set self.x (clamp (+ self.x (* dx speed dt)) 0 800))
  (set self.y (clamp (+ self.y (* dy speed dt)) 0 600)))

(λ collides? [a b]
  (and (< a.x (+ b.x b.w)) (< b.x (+ a.x a.w))
       (< a.y (+ b.y b.h)) (< b.y (+ a.y a.h))))

(macro unless [cond ...]
  `(when (not ,cond) ,...))

(fn love.update [dt]
  (let [dx (if (love.keyboard.isDown :left) -1 (love.keyboard.isDown :right) 1 0)]
    (player:move dx 0 dt))
  (each [i enemy (ipairs enemies)]
    (when (collides? player enemy)
      (set score (+ score 1))
      (table.remove enemies i))))

(fn love.draw []
  (love.graphics.print (.. "Score: " score "\n") 10 10)
  (unless (= score 0)
    (print (string.format "%d points" score))))

(local squares (icollect [_ n (ipairs [1 2 3 0xFF])] (* n n)))
(local lookup (collect [k v (pairs {:a 1 :b 2})] (values v k)))
(print (length squares) (. lookup 1) #(+ $1 1) nil true)
