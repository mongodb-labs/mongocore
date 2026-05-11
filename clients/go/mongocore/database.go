package mongocore

// Database provides access to collections within a database.
type Database struct {
	client *Client
	name   string
}

// Name returns the database name.
func (d *Database) Name() string {
	return d.name
}

// Collection returns a collection handle.
func (d *Database) Collection(name string) *Collection {
	return &Collection{
		client:   d.client,
		database: d.name,
		name:     name,
	}
}
