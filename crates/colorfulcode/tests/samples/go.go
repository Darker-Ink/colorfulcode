// Package store keeps users in memory.
package store

import (
	"errors"
	"fmt"
	"net/http"
)

const MaxUsers = 1_000

var ErrNotFound = errors.New("user not found")

type Role int

const (
	Admin Role = iota
	Guest
)

// User is a registered account.
type User struct {
	ID    int64  `json:"id"`
	Name  string `json:"name"`
	Roles []Role
	Next  *User
}

type Store struct {
	users map[int64]*User
}

func NewStore() *Store {
	return &Store{users: make(map[int64]*User, 16)}
}

func (s *Store) Get(id int64) (*User, error) {
	u, ok := s.users[id]
	if !ok {
		return nil, fmt.Errorf("get %d: %w", id, ErrNotFound)
	}
	return u, nil
}

func Map[T, U any](xs []T, f func(T) U) []U {
	out := make([]U, 0, len(xs))
	for _, x := range xs {
		out = append(out, f(x))
	}
	return out
}

func handler(w http.ResponseWriter, r *http.Request) {
	w.WriteHeader(http.StatusOK)
	fmt.Fprintf(w, "hello, %s!\n", r.URL.Path[1:])
	go func() { _ = 'x' + 0x1F }()
}
