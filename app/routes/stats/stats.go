package stats

import (
	"net/http"
	"net/http/httputil"

	"github.com/jackc/pgx/v4/pgxpool"
)

type Service struct {
	db                *pgxpool.Pool
	badgeReverseProxy *httputil.ReverseProxy
}

func NewService(db *pgxpool.Pool, reverseProxy *httputil.ReverseProxy) *Service {
	return &Service{db, reverseProxy}
}

func (s *Service) Handler(w http.ResponseWriter, r *http.Request) {
	s.badgeReverseProxy.ServeHTTP(w, r)
}
