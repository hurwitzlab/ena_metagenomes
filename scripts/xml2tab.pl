#!/usr/bin/env perl

use strict;
use autodie;
use utf8;
use feature 'say';
use Data::Dump 'dump';
use DateTime::Format::Excel;
use DateTime;
use File::Find::Rule;
use File::Basename 'basename';
use Getopt::Long;
use Pod::Usage;
use Readonly;
use Regexp::Common;
use Time::ParseDate 'parsedate';
use XML::LibXML;

my %SHORT_MONTH = (
    jan => 1, 
    feb => 2,
    mar => 3,
    apr => 4,
    may => 5,
    jun => 6,
    jul => 7,
    aug => 8,
    sep => 9,
    oct => 10,
    nov => 11,
    dec => 12,
);
my %LONG_MONTH = (
    january    => 1, 
    february   => 2,
    march      => 3,
    april      => 4,
    may        => 5,
    june       => 6,
    july       => 7,
    august     => 8,
    september  => 9,
    october    => 10,
    november   => 11,
    december   => 12,
);
my $SHORT_MONTHS     = join '|', keys %SHORT_MONTH;
my $LONG_MONTHS      = join '|', keys %LONG_MONTH;
my $SHORT_MONTH_YEAR = qr/^($SHORT_MONTHS)[^-]*[,-]\s*(\d{4})$/i;
my $LONG_MONTH_YEAR  = qr/^($LONG_MONTHS)[,-]\s*(\d{4})$/i;
my $LONG_MONTH_YEAR2 = qr/^($LONG_MONTHS)\-(?:$LONG_MONTHS)\s+(\d{4})$/i;

my $EVENT_DATE_TIME = qr/
    ^
    event
    [\s_]
    date
    [\s\/_]
    time
    [\s_]
    (?:start)
    $
    /xmsi;
    
my $COLLECTION_DATE = qr/
    ^
    (?:event|collection)
    [\s_]
    date
    (?:\/time)?
    $
    /ixms;

my $DEPTH = qr/^(?:geographic(?:al)? location [(])?depth[)]?/;
my $GEO_LAT_LON =
  qr/^(?:geographic(?:al)? location [(])?latitude and longitude(?:[)])?/;
my $GEO_LAT    = qr/^(?:geographic(?:al)? location [(])?lat(?:itude)?[)]?/;
my $GEO_LON1   = qr/^(?:geographic(?:al)? location [(])?lon(?:gitude)?[)]?/;
my $GEO_LON2   = qr/^longitude(?:_deg|\s+start)?/;
my $LAT_LON_RE = qr/
    ^
    (?:lat:?\s*)?
    ($RE{'num'}{'real'})
    (?:\s*([NS]))?
    (?:_|\s+|\s*,\s*)
    (?:long:?\s*)?
    ($RE{'num'}{'real'})
    (?:\s*([EW]))?
    (?:,\s+decimal\s+degrees)?
    $
    /xmsi;

my $LAT_LON_DEGREE_MINUTE = qr/
    ^
    (\d+)
    [º]?
    \s*
    ($RE{'num'}{'real'}|$RE{'num'}{'int'})
    [']?
    (?:\s*[NS])?
    [^\w]+
    (\d+)
    [º]?
    \s*
    ($RE{'num'}{'real'}|$RE{'num'}{'int'})
    [']?
    (?:\s*[EW])?
    /xms;
    #[^\d]+
    #(?:_|\s+|\s*,\s*)

my $LAT_LON_DEGREE_MINUTE2 = qr/
    ^
    (-)?
    (\d+)
    \.
    (\d+)
    [']
    ($RE{'num'}{'real'})
    ["]
    \s+
    (-)?
    (\d+)
    \.
    (\d+)
    [']
    ($RE{'num'}{'real'})
    ["]
    $
    /xms;

my $LAT_LON_INT      = qr/^($RE{'num'}{'int'})\s+($RE{'num'}{'int'})$/;
my $LAT_LON_FLOAT    = qr/^($RE{'num'}{'real'})\s+($RE{'num'}{'real'})$/;
my $LAT_LON_MIN_SEC = qr/
    ^
    \s*
    (\d+)
    [ºÁ]?
    (\d+)
    [,’']
    ($RE{'num'}{'real'})
    ['’]{2}?
    (?:[\s']*([NS]))?
    (?:\s+|\s*,\s*)
    (\d+)
    [ºÁ]?
    (\d+)
    [,’']
    ($RE{'num'}{'real'})
    ['’]{2}?
    (?:[\s']*([EW]))?
    $
    /xms;

my $LAT_LON_NO_SEPARATOR = qr/
    ^
    ([+-])?
    (\d+[.]\d+)
    ([+-])
    (\d+[.]\d+)
    /xms;

my $COORD_DEGREE_MINUTE = qr/
    ^
    \s*
    ([-]?)
    (\d+)
    [°]?
    \s+
    ($RE{'num'}{'real'})
    $
    /xms;

my $COORD_RE = qr/
    ^
    \s*
    ($RE{'num'}{'real'}|$RE{'num'}{'int'})
    (?:\s*[º])?
    (?:\s*([NSEW]))?
    /xms;

my $COORD_COMMA = qr/
    ^
    \s*
    (\d+)
    ,
    (\d+)
    $
    /xms;

my $COORD_DMS_INVERT = qr/
    ^
    \s*
    (\d+)
    ([NSEW])
    \s+
    (\d+)
    degrees
    \s+
    (\d{2})
    [']
    \s+
    ($RE{'num'}{'real'})
    ["]
    $
    /xms;

my $COORD_DMS_INVERT2 = qr/
    ^
    ([NSEW])
    \s+
    (\d+)
    \s+
    degrees
    \s+
    (\d{2})
    [']
    \s+
    ($RE{'num'}{'real'})
    ["]?
    $
    /xms;

my $COORD_DHM = qr/
    ^
    \s*
    (\d+)
    (\d+)
    (?:[º\s])
    \s*
    (\d+)
    [,']
    ($RE{'num'}{'real'})
    (?:"|'{2})?
    (?:[\s']*([NSEW]))?
    $
    /xms;

my $COORD_DDMMSS = qr/
    ^
    \s*
    (\d+)
    (\d+)
    (\d{2})
    (?:['](\d+))?
    (?:\s*([NSEW]))?
    $
    /xms;

my $COORD_TICKS = qr/
    ^
    \s*
    (\d+)
    (\d+)
    \s+
    (\d+)
    [']{1}
    \s*
    (\d+)
    [']{2}
    (?:\s*([NSEW]))?
    /xms;

main();

# --------------------------------------------------
sub get_args {
    my %args;
    GetOptions( \%args, 'file|f=s', 'dir|d=s', 'out|o=s', 'help', 'man', )
      or pod2usage(2);

    return %args;
}

# --------------------------------------------------
sub main {
    my %args = get_args();

    if ( $args{'help'} || $args{'man'} ) {
        pod2usage(
            {
                -exitval => 0,
                -verbose => $args{'man'} ? 2 : 1
            }
        );
    }

    unless ( $args{'dir'} || $args{'file'} ) {
        pod2usage('Need either --file or --dir');
    }

    my @files;

    if ( my $file = $args{'file'} ) {
        push @files, $file;
    }
    elsif ( my $in_dir = $args{'dir'} ) {
        die "-d '$in_dir' is not a directory" unless -d $in_dir;

        @files = File::Find::Rule->file()->name('*.xml')->in($in_dir);
    }

    die "No input files\n" unless @files;

    my $out_file = $args{'out'} || 'data.tab';
    open my $out_fh, '>', $out_file;

    my @req_flds = qw[sample collection_date latitude longitude depth];
    my @opt_flds = qw[runs];

    say $out_fh join "\t", @req_flds, @opt_flds;

    my ( $i, $exported ) = ( 0, 0 );
    for my $file (@files) {
        printf "%6d %s\n", ++$i, basename($file);
        if ( process( $file, $out_fh, \@req_flds, \@opt_flds ) > 0 ) {
            $exported++;
        }
    }

    printf "Done, exported %s of %s file%s into '%s'.\n",
      $exported, $i, $i == 1 ? '' : 's', $out_file;
}

# --------------------------------------------------
sub process {
    my ( $file, $fh, $req_flds, $opt_flds ) = @_;

    my $dom = XML::LibXML->load_xml(location => $file);

    #say dump($sample);

    my $acc = $dom->findnodes('./SAMPLE/IDENTIFIERS/PRIMARY_ID');
    unless ($acc) {
        say STDERR "Error in file '$file': No accession";
        return 0;
    }

    my @runs;
    for my $link ($dom->findnodes('./SAMPLE/SAMPLE_LINKS/SAMPLE_LINK')) {
        my $db = $link->findvalue('./XREF_LINK/DB');
        if ($db eq 'ENA-RUN') {
            my $id = $link->findvalue('./XREF_LINK/ID');
            push @runs, $id;
        }
    }

    my %ena = ( sample => "$acc" );
    my @attrs = $dom->findnodes('./SAMPLE/SAMPLE_ATTRIBUTES/SAMPLE_ATTRIBUTE');
    for my $attr (@attrs) {
        my $tag = lc($attr->findvalue('TAG'));
        my $val = $attr->findvalue('VALUE');
        next unless defined $tag && defined $val && $val ne '';

        #
        # Collection Date
        #
        if ( 
            (
                   $tag =~ $COLLECTION_DATE 
                || $tag =~ $EVENT_DATE_TIME
                || lc($tag) eq 'date'
                || lc($tag) eq 'collection_timestamp'
            ) 
            && $val =~ /\d+/ 
        ) {
            $val =~ s/_/ /g;

            # Excel format (5 digits)
            if ( $val =~ /^\d{5}$/ ) {
                if (my $dt = DateTime::Format::Excel->parse_datetime($val)) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
            }
            # E.g., 2015-01, 2015-01/2015-02
            elsif ( $val =~ /^(\d{4})[-](\d{1,2})(?:\/.+)?$/ ) {
                my $dt = DateTime->new(
                    year  => $1,
                    month => $2,
                    day   => '01',
                );

                if ($dt) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
            }
            # E.g., Dec-2015
            elsif ( $val =~ $SHORT_MONTH_YEAR ) {
                my $dt = DateTime->new(
                    year  => $2,
                    month => $SHORT_MONTH{lc $1},
                    day   => '01',
                );

                if ($dt) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
            }
            # E.g., March-2017
            elsif ( $val =~ $LONG_MONTH_YEAR ) {
                my $dt = DateTime->new(
                    year  => $2,
                    month => $LONG_MONTH{lc $1},
                    day   => '01',
                );

                if ($dt) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
            }
            # E.g., March-April 2017
            elsif ( $val =~ $LONG_MONTH_YEAR2 ) {
                my $dt = DateTime->new(
                    year  => $2,
                    month => $LONG_MONTH{lc $1},
                    day   => '01',
                );

                if ($dt) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
            }
            # E.g., July of 2011
            elsif ( $val =~ /^($LONG_MONTHS)\s+of\s+(\d{4})$/i ) {
                my $dt = DateTime->new(
                    year  => $2,
                    month => $LONG_MONTH{lc $1},
                    day   => '01',
                );

                if ($dt) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
            }
            # E.g., 20100910
            elsif ( $val =~ /^(\d{4})(\d{2})(\d{2})$/ ) {
                my ($year, $month, $day) = ($1, $2, $3);
                if (
                    ($year > 1900 && $year < 2019)
                    &&
                    ($month >= 1 && $month <= 12)
                    &&
                    ($day >= 1 && $day <= 31)
                ) {
                    my $dt = DateTime->new(
                        year  => $1,
                        month => $2,
                        day   => $3,
                    );

                    if ($dt) {
                        $ena{'collection_date'} = $dt->iso8601();
                    }
                }
            }
            # E.g., "2008 August"
            elsif ( $val =~ /^(\d{4})\s+($LONG_MONTHS)/i ) {
                my $dt = DateTime->new(
                    year  => $1,
                    month => $LONG_MONTH{lc $2},
                    day   => 01,
                );

                if ($dt) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
            }
            # E.g., 12/06, 12/06-1/07
            elsif ( $val =~ m{^(\d{1,2})/(\d{2})(?:[-]\d{1,2}/\d{2})?$} ) {
                my $month = $1;
                my $year  = '20' . $2;
                my $dt = DateTime->new(
                    year  => $year,
                    month => $2,
                    day   => 01,
                );

                if ($dt) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
            }
            # E.g., 2017-06-16Z
            elsif ( $val =~ m/^(\d{4})[-](\d{2})[-](\d{2})Z$/ ) {
                my $dt = DateTime->new(
                    year  => $1,
                    month => $2,
                    day   => $3
                );

                if ($dt) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
            }
            elsif ( $val =~ /^(\d{4}-\d{2}-\d{2}T\d+:\d+:\d+)/ ) {    # ISO
                $ena{'collection_date'} = $1;
            }
            elsif ( $val =~ m!^(\d{4})-(\d{2})-(\d{2})/\d{4}-\d{2}-\d{2}! ) {
                my $dt = DateTime->new(
                    year  => $1,
                    month => $2,
                    day   => $3,
                );

                if ($dt) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
            }
            else {
                my $dt;
                eval {
                    my $seconds = parsedate($val);
                    $dt = DateTime->from_epoch( epoch => $seconds );
                };

                if ($dt) {
                    $ena{'collection_date'} = $dt->iso8601();
                }
                else {
                    say STDERR "Error parsing date '$val' ($acc)";    #: $@";
                }
            }
        }
        #
        # Latitude and Longitude
        #
        elsif ( 
            ( $tag =~ /^lat[\s_]lon$/ || $tag =~ $GEO_LAT_LON )
            && $val =~ /\d+/ )
        {
            $val =~ s/´/'/g;

            if ( $val =~ $LAT_LON_MIN_SEC ) {
                my (
                    $lat_degree, $lat_min, $lat_sec, $ns,
                    $lon_degree, $lon_min, $lon_sec, $ew
                ) = ( $1, $2, $3, $4, $5, $6, $7, $8 );
                my $lat = h2d( $lat_degree, $lat_min, $lat_sec );
                my $lon = h2d( $lon_degree, $lon_min, $lon_sec );
                $lat *= $ns eq 'S' ? -1 : 1;
                $lon *= $ew eq 'W' ? -1 : 1;
                ( $ena{'latitude'}, $ena{'longitude'} ) = ( $lat, $lon );
            }
            elsif ( $val =~ $LAT_LON_RE ) {
                my ( $lat, $ns, $lon, $ew ) = ( $1, $2, $3, $4 );
                $lat *= $ns eq 'S' ? -1 : 1;
                $lon *= $ew eq 'W' ? -1 : 1;
                ( $ena{'latitude'}, $ena{'longitude'} ) = ( $lat, $lon );
            }
            elsif ( $val =~ $LAT_LON_DEGREE_MINUTE ) {
                my ( $lat_degree, $lat_min, $ns, 
                    $lon_degree, $lon_min, $ew ) = ( $1, $2, $3, $4, $5, $6 );
                my $lat = h2d( $lat_degree, $lat_min, 0);
                my $lon = h2d( $lon_degree, $lon_min, 0);
                $lat *= $ns eq 'S' ? -1 : 1;
                $lon *= $ew eq 'W' ? -1 : 1;
                ( $ena{'latitude'}, $ena{'longitude'} ) = ( $lat, $lon );
            }
            # 11.46'45.7" 93.01'22.3"
            elsif ( $val =~ $LAT_LON_DEGREE_MINUTE2 ) {
                my ( $lat_dir, $lat_degree, $lat_min, $lat_sec,
                    $lon_dir, $lon_degree, $lon_min, $lon_sec ) = 
                    ( $1, $2, $3, $4, $5, $6, $7, $8 );
                my $lat = h2d( $lat_degree, $lat_min, $lat_sec);
                my $lon = h2d( $lon_degree, $lon_min, $lon_sec);
                $lat *= $lat_dir eq '-' ? -1 : 1;
                $lon *= $lon_dir eq '-' ? -1 : 1;
                ( $ena{'latitude'}, $ena{'longitude'} ) = ( $lat, $lon );
            }
            elsif ( $val =~ $LAT_LON_FLOAT ) {
                ( $ena{'latitude'}, $ena{'longitude'} ) = ( $1, $2 );
            }
            elsif ( $val =~ $LAT_LON_INT ) {
                ( $ena{'latitude'}, $ena{'longitude'} ) = ( $1, $2 );
            }
            elsif ( $val =~ $LAT_LON_NO_SEPARATOR ) {
                my ( $lat_sign, $lat, $lon_sign, $lon ) = ( $1, $2, $3, $4 );
                $lat *= $lat_sign eq '-' ? -1 : 1;
                $lon *= $lon_sign eq '-' ? -1 : 1;
                ( $ena{'latitude'}, $ena{'longitude'} ) = ( $lat, $lon );
            }
            else {
                say STDERR "Error parsing lat_lon '$val' ($acc)";
            }
        }
        #
        # Latitude
        #
        elsif ( ( $tag eq 'latitude' || $tag =~ $GEO_LAT )
            && $val =~ /\d+/ )
        {
            $val =~ s/´/'/g;

            if ( $attr->{'UNITS'} eq 'DDMMSS' && $val =~ $COORD_DDMMSS ) {
                my ( $degree, $min, $sec, $ns ) = ( $1, $2, $3, $4 );
                my $lat = h2d( $degree, $min, $sec );
                $lat *= $ns eq 'S' ? -1 : 1;
                $ena{'latitude'} = $lat;
            }
            elsif ( $val =~ $COORD_DMS_INVERT ) {
                my ( $degree, $ns, $min, $sec ) = ( $1, $2, $3, $4 );
                my $lat = h2d( $degree, $min, $sec );
                $lat *= $ns eq 'S' ? -1 : 1;
                $ena{'latitude'} = $lat;
            }
            elsif ( $val =~ $COORD_DMS_INVERT2 ) {
                my ( $ns, $degree, $min, $sec ) = ( $1, $2, $3, $4 );
                my $lat = h2d( $degree, $min, $sec );
                $lat *= $ns eq 'S' ? -1 : 1;
                $ena{'latitude'} = $lat;
            }
            elsif ( $val =~ $COORD_DHM ) {
                my ( $degree, $hour, $min_sec, $ns ) = ( $1, $2, $3, $4 );
                my $lat = h2d( $degree, $hour, $min_sec );
                $lat *= $ns eq 'S' ? -1 : 1;
                $ena{'latitude'} = $lat;
            }
            elsif ( $val =~ $COORD_RE ) {
                my ( $lat, $ns ) = ( $1, $2 );
                $lat *= $ns eq 'S' ? -1 : 1;
                $ena{'latitude'} = $lat;
            }
            elsif ( $val =~ $COORD_TICKS ) {
                my ( $degree, $min, $sec, $ew ) = ( $1, $2, $3, $4 );
                my $lat = h2d( $degree, $min, $sec );
                $lat *= $ew eq 'W' ? -1 : 1;
                $ena{'latitude'} = $lat;
            }
            elsif ( $val =~ $COORD_COMMA ) {
                $ena{'latitude'} = join '.', $1, $2;
            }
            elsif ( $val =~ $COORD_DEGREE_MINUTE ) {
                my ( $neg, $degree, $min ) = ( $1, $2, $3 );
                my $lat = h2d( $degree, $min, 0 );
                if ($neg eq '-') {
                    $lat *= -1;
                }
                $ena{'latitude'} = $lat;
            }
            else {
                say STDERR "Error parsing latitude '$val' ($acc)";
            }
        }
        #
        # Longitude
        #
        elsif ( ( $tag =~ $GEO_LON1 || $tag =~ $GEO_LON2 )
            && $val =~ /\d+/ )
        {
            $val =~ s/´/'/g;

            if ( $attr->{'UNITS'} eq 'DDMMSS' && $val =~ $COORD_DDMMSS ) {
                my ( $degree, $min, $sec, $ew ) = ( $1, $2, $3, $4 );
                my $lon = h2d( $degree, $min, $sec );
                $lon *= $ew eq 'W' ? -1 : 1;
                $ena{'longitude'} = $lon;
            }
            elsif ( $val =~ $COORD_DMS_INVERT ) {
                my ( $degree, $ew, $min, $sec ) = ( $1, $2, $3, $4 );
                my $lon = h2d( $degree, $min, $sec );
                $lon *= $ew eq 'W' ? -1 : 1;
                $ena{'longitude'} = $lon;
            }
            elsif ( $val =~ $COORD_DMS_INVERT2 ) {
                my ( $ew, $degree, $min, $sec ) = ( $1, $2, $3, $4 );
                my $lon = h2d( $degree, $min, $sec );
                $lon *= $ew eq 'W' ? -1 : 1;
                $ena{'longitude'} = $lon;
            }
            elsif ( $val =~ $COORD_DHM ) {
                my ( $degree, $hour, $min_sec, $ew ) = ( $1, $2, $3, $4 );
                my $lon = h2d( $degree, $hour, $min_sec );
                $lon *= $ew eq 'W' ? -1 : 1;
                $ena{'longitude'} = $lon;
            }
            elsif ( $val =~ $COORD_RE ) {
                my ( $lon, $ew ) = ( $1, $2 );
                $lon *= $ew eq 'W' ? -1 : 1;
                $ena{'longitude'} = $lon;
            }
            elsif ( $val =~ $COORD_TICKS ) {
                my ( $degree, $min, $sec, $ns ) = ( $1, $2, $3, $4 );
                my $lon = h2d( $degree, $min, $sec );
                $lon *= $ns eq 'S' ? -1 : 1;
                $ena{'longitude'} = $lon;
            }
            elsif ( $val =~ $COORD_COMMA ) {
                $ena{'longitude'} = join '.', $1, $2;
            }
            elsif ( $val =~ $COORD_DEGREE_MINUTE ) {
                my ( $neg, $degree, $min ) = ( $1, $2, $3 );
                my $lon = h2d( $degree, $min, 0 );
                if ($lon eq '-') {
                    $lon *= -1;
                }
                $ena{'longitude'} = $lon;
            }
            else {
                say STDERR "Error parsing longitude '$val' ($acc)";
            }
        }
        #
        # Depth
        #
        elsif ( $tag =~ $DEPTH && $val =~ /\d+/ ) {
            if (
                $val =~ m/($RE{'num'}{'real'}|$RE{'num'}{'int'})(?:\s+(\w+))?/ )
            {
                my ( $num, $unit ) = ( $1, $2 );
                $unit ||= lc( $attr->{'UNITS'} );
                if ($unit) {
                    if ( $unit eq 'cm' ) {
                        $num *= 10;
                    }
                    elsif ( $unit =~ /^m(eters?)?/ ) {
                        ;    # do nothing
                    }
                    else {
                        $num = undef;
                        say STDERR "depth '$val' unit ($unit) not cm|m\n";
                    }
                }

                if ( defined $num ) {
                    $ena{'depth'} = $num;
                }
            }
            else {
                say STDERR "Error parsing depth '$val' ($acc)";
                return 0;
            }

            #if (my ($depth, $unit) = split /\s+/, $val) {
            #    if ($unit && $unit ne 'meter') {
            #        die "unit ($unit) not meter\n";
            #    }
            #}
            #
            #$depth ||= $val;
            #if ($depth =~ $RE{'num'}{'int'} || $depth =~ $RE{'num'}{'real'}) {
            #    $ena{'depth'} = $depth;
            #}
        }
    }

    $ena{'depth'} //= -1;
    $ena{'runs'} = join ', ', @runs;

    #say STDERR dump(\%ena);

    if ( my @missing = grep { !defined $ena{$_} } @$req_flds ) {
        say STDERR "Rejected $acc missing ", join ', ', @missing;

        #say dump(\%ena);
        #say STDERR "Rejected $acc"; #, dump(\%ena);
        #say STDERR "Rejected ", dump(\%ena);
        return 0;
    }
    else {
        say $fh join "\t", map { $ena{$_} } @$req_flds, @$opt_flds;
        return 1;
    }
}

# --------------------------------------------------
sub h2d {
    my ( $degree, $min, $sec ) = @_;
    return $degree + $min / 60 + $sec / 3600;
}

# --------------------------------------------------
sub parse_date {
    my $val = shift;
    $val =~ s/_/ /g;

    if ( $val =~ /^\d{4}-\d{2}-\d{2}T\d+:\d+\d+/ ) {    # ISO
        return $val;
    }
    else {
        my $dt;
        eval {
            my $seconds = parsedate($val);
            $dt = DateTime->from_epoch( epoch => $seconds );
        };

        if ($dt) {
            return $dt->iso8601();
        }
        elsif ($@) {
            say STDERR "Error parsing date '$val'" if $@;
        }
    }
}

__END__

# --------------------------------------------------

=pod

=head1 NAME

xml2tab.pl - a script

=head1 SYNOPSIS

  xml2tab.pl -f file
  xml2tab.pl -d in_dir [-o out.tab] 2>err

Options:

  -f|--file  Input file
  -d|--dir   Input directory
  -o|--out   Output filename (data.tab)

  --help     Show brief help and exit
  --man      Show full documentation

=head1 DESCRIPTION

Processes the directory containing all the small XML files 
from "xml-breakup.pl" and creates "-o" output tab-delimited file.

=head1 AUTHOR

Ken Youens-Clark E<lt>kyclark@email.arizona.eduE<gt>.

=head1 COPYRIGHT

Copyright (c) 2018 Hurwitz Lab

This module is free software; you can redistribute it and/or
modify it under the terms of the GPL (either version 1, or at
your option, any later version) or the Artistic License 2.0.
Refer to LICENSE for the full license text and to DISCLAIMER for
additional warranty disclaimers.

=cut
