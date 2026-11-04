;;; tidy-mode.el --- Remove trailing whitespace on save -*- lexical-binding: t -*-

;;; Commentary:
;; A small minor mode with a customizable option.

;;; Code:

(require 'cl-lib)

(defgroup tidy nil
  "Keep buffers tidy."
  :group 'editing)

(defcustom tidy-ignored-modes '(markdown-mode diff-mode)
  "Major modes where tidying is skipped."
  :type '(repeat symbol)
  :group 'tidy)

(defvar tidy--count 0
  "Number of lines cleaned in this session.")

(defun tidy--should-run-p ()
  "Return non-nil when the current buffer should be tidied."
  (not (memq major-mode tidy-ignored-modes)))

(defun tidy-buffer ()
  "Delete trailing whitespace and report how many lines changed."
  (interactive)
  (when (tidy--should-run-p)
    (save-excursion
      (goto-char (point-min))
      (while (re-search-forward "[ \t]+$" nil t)
        (replace-match "")
        (cl-incf tidy--count)))
    (message "Tidied %d lines\n" tidy--count)))

;;;###autoload
(define-minor-mode tidy-mode
  "Tidy buffers before saving."
  :lighter " Tidy"
  (if tidy-mode
      (add-hook 'before-save-hook #'tidy-buffer nil t)
    (remove-hook 'before-save-hook #'tidy-buffer t)))

(global-set-key (kbd "C-c t") #'tidy-buffer)
(defconst tidy-chars (list ?a ?\n ?\C-x ?\s))
(setq tidy--count (+ tidy--count 1 -1 2.5 #x1F))

(provide 'tidy-mode)
;;; tidy-mode.el ends here
