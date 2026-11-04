# Summarise a data frame
#' @param df a data frame
library(dplyr)

THRESHOLD <- 0.05
normalize <- function(x, na.rm = TRUE) {
  rng <- range(x, na.rm = na.rm)
  (x - rng[1]) / (rng[2] - rng[1])
}

square = function(n) n^2L
add <- \(a, b) a + b

df <- data.frame(id = 1:10, value = rnorm(10, mean = 5))
df$scaled <- normalize(df$value)
result <- df %>%
  filter(value > THRESHOLD) %>%
  mutate(flag = ifelse(scaled > 0.5, "high", 'low'))

for (i in seq_len(nrow(df))) {
  if (is.na(df$value[i])) next
  cat(sprintf("Row %d: %.2f\n", i, df$value[i]))
}

stopifnot(all(df$id %in% 1:10), !is.null(result))
x <- c(TRUE, FALSE, NA, Inf, 1e-3, 0x1F)
`my var` <- list(a = 1, b = NULL)
print(dplyr::n_distinct(df$id))
