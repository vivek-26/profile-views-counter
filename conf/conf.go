package conf

import (
	"github.com/kelseyhightower/envconfig"
)

// Config has all app configurations
type Config struct {
	Port           int               `default:"9000"`
	DatabaseURL    string            `split_words:"true" required:"true"`
	ServiceUserMap map[string]string `split_words:"true" required:"true"`
}

// Load reads all env vars needed by application
func Load() (*Config, error) {
	var config Config
	err := envconfig.Process("", &config)
	return &config, err
}
